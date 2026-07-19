//! cdi-probe — CLI diagnostic tool for LCC node discovery and CDI download timing.
//!
//! Exercises the same `lcc-rs` code paths as the Bowties Tauri app so hardware
//! bugs can be reproduced without the frontend in the loop. Useful for
//! sweeping `post_ack_delay_ms`, chunk timeouts, and back-to-back reliability
//! against a real peer (e.g. SPROG USB-LCC).

use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand, ValueEnum};
use lcc_rs::{
    FrameEncoding, GridConnectFrame, GridConnectSerialTransport, LccConnection, MemoryReadConfig,
    NodeID, PeerError, PeerSessionRegistry, SerialFlowControl, MTI,
};
use serde::Serialize;
use tokio::sync::Mutex;

// ─── CLI definition ─────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "cdi-probe",
    version,
    about = "LCC discovery and CDI download timing probe",
    long_about = "Exercises lcc-rs transport + discovery + CDI-download paths \
                  from the command line. Use to reproduce hardware timing \
                  bugs (e.g. SPROG buffer-pressure DRs) without the Tauri UI."
)]
struct Cli {
    /// Serial port (e.g. COM8, /dev/ttyUSB0).
    #[arg(long, global = true, default_value = "COM8")]
    port: String,

    /// Serial baud rate. Defaults to 460800 for SPROG USB-LCC / PI-LCC.
    /// RR-CirKits Buffer-LCC uses 57600; MERG CAN-RS uses 57600 too.
    #[arg(long, global = true, default_value_t = 460800)]
    baud: u32,

    /// Flow control. Defaults to `hardware` for SPROG USB-LCC / PI-LCC.
    /// RR-CirKits Buffer-LCC and MERG CAN-RS need `none`.
    #[arg(long, global = true, value_enum, default_value_t = FlowArg::Hardware)]
    flow: FlowArg,

    /// GridConnect frame encoding.
    #[arg(long, global = true, value_enum, default_value_t = EncodingArg::Standard)]
    encoding: EncodingArg,

    /// Our own node ID (dotted or contiguous hex).
    #[arg(long, global = true, default_value = "05.01.01.01.A2.FE")]
    our_node_id: String,

    /// Milliseconds to wait after opening the serial port before writing
    /// the first alias-allocation frame. Gives the USB-CDC adapter time
    /// to complete its DTR-assert / initialization handshake before we
    /// start blasting frames at it.
    #[arg(long, global = true, default_value_t = 200)]
    open_settle_ms: u64,

    /// Milliseconds to wait after discovery replies stop before creating
    /// peer sessions (gives the registry's spawn-watcher time to observe
    /// the same frames and spawn actors).
    #[arg(long, global = true, default_value_t = 100)]
    session_settle_ms: u64,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Discover nodes on the bus and print them with reply timing.
    Discover {
        /// How long to collect Verified Node replies (ms).
        #[arg(long, default_value_t = 500)]
        timeout_ms: u64,
    },
    /// Diagnostic: dump every inbound frame for a fixed duration after
    /// sending a global Verify Node ID probe. Use when discovery finds
    /// zero nodes to see whether the peer is actually replying.
    Raw {
        /// How long to listen (ms).
        #[arg(long, default_value_t = 2000)]
        duration_ms: u64,

        /// Skip the VerifyNodeGlobal probe at startup — listen only for
        /// spontaneous bus traffic.
        #[arg(long)]
        no_probe: bool,

        /// Include our own 6-byte NodeID as the VerifyNodeGlobal payload
        /// instead of sending it with an empty payload.
        #[arg(long)]
        with_payload: bool,
    },
    /// Download CDI from a target node N times and report per-run stats.
    Cdi {
        /// Target node ID (dotted or contiguous hex).
        #[arg(long)]
        node: String,

        /// Number of back-to-back downloads.
        #[arg(long, default_value_t = 5)]
        iterations: usize,

        /// Post-ACK pacing delay (ms) — the parameter we're tuning.
        #[arg(long, default_value_t = 100)]
        post_ack_delay_ms: u64,

        /// Per-chunk read timeout (ms).
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,

        /// Max retries on resend-OK DR.
        #[arg(long, default_value_t = 3)]
        max_retries: u32,

        /// Discovery collection window before starting downloads (ms).
        #[arg(long, default_value_t = 500)]
        discover_timeout_ms: u64,

        /// Emit JSON records instead of a human-readable table (one JSON
        /// object per iteration on stdout, summary at end).
        #[arg(long)]
        json: bool,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum FlowArg {
    None,
    Hardware,
    Software,
}

impl From<FlowArg> for SerialFlowControl {
    fn from(f: FlowArg) -> Self {
        match f {
            FlowArg::None => SerialFlowControl::None,
            FlowArg::Hardware => SerialFlowControl::Hardware,
            FlowArg::Software => SerialFlowControl::Software,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum EncodingArg {
    Standard,
    Merg,
}

impl From<EncodingArg> for FrameEncoding {
    fn from(e: EncodingArg) -> Self {
        match e {
            EncodingArg::Standard => FrameEncoding::Standard,
            EncodingArg::Merg => FrameEncoding::MergCanRs,
        }
    }
}

// ─── Records for JSON output ────────────────────────────────────────────────

#[derive(Serialize)]
struct IterationRecord {
    iteration: usize,
    #[serde(flatten)]
    outcome: IterationOutcome,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
enum IterationOutcome {
    Ok {
        total_bytes: usize,
        chunks: usize,
        total_duration_ms: u64,
        total_retries: usize,
        chunk_min_ms: u32,
        chunk_max_ms: u32,
        chunk_mean_ms: u32,
        chunk_p95_ms: u32,
    },
    Err {
        error_kind: String,
        detail: String,
        elapsed_ms: u128,
    },
}

#[derive(Serialize)]
struct SummaryRecord {
    iterations: usize,
    successes: usize,
    failures: usize,
    total_duration_ms_min: Option<u64>,
    total_duration_ms_median: Option<u64>,
    total_duration_ms_mean: Option<u64>,
    total_duration_ms_p95: Option<u64>,
    total_duration_ms_max: Option<u64>,
    total_retries_across_runs: usize,
    post_ack_delay_ms: u64,
    timeout_ms: u64,
}

// ─── Entry point ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let our_node_id = NodeID::from_hex_string(&cli.our_node_id)
        .map_err(|e| format!("invalid --our-node-id: {}", e))?;

    // Match the app's construction sequence.
    eprintln!(
        "[cdi-probe] Opening {} @ {} baud, flow={:?}, encoding={:?}",
        cli.port, cli.baud, cli.flow, cli.encoding
    );
    let transport = GridConnectSerialTransport::open(
        &cli.port,
        cli.baud,
        cli.flow.into(),
        cli.encoding.into(),
    )
    .await?;

    // Give the USB-CDC adapter time to complete its post-DTR-assertion
    // initialization before we start blasting frames at it. Bowties'
    // long-lived Tauri process gets this time "for free" from framework
    // startup; a short-lived CLI does not.
    eprintln!("[cdi-probe] Port open. Waiting {}ms for adapter to settle...", cli.open_settle_ms);
    tokio::time::sleep(Duration::from_millis(cli.open_settle_ms)).await;

    let connection = LccConnection::connect_with_dispatcher_and_transport(
        Box::new(transport),
        our_node_id,
    )
    .await?;

    let (transport_handle, our_alias) = {
        let conn = connection.lock().await;
        let h = conn
            .transport_handle()
            .cloned()
            .ok_or("connection has no transport handle")?;
        (h, conn.our_alias().value())
    };
    eprintln!("[cdi-probe] Connected. Our alias = 0x{:03X}", our_alias);

    // Match the app's post-connect settling: registry, responders, event
    // router subscriptions, and SNIP setup all happen synchronously before
    // the user can click Probe. That takes several hundred milliseconds of
    // wall-clock time in the app, which may matter for the SPROG's ability
    // to see subsequent frames. Sleep here to reproduce that gap.
    tokio::time::sleep(Duration::from_millis(500)).await;
    eprintln!("[cdi-probe] Post-connect settling delay complete.");

    // Registry watches inbound frames and spawns per-peer sessions
    // opportunistically.
    let registry = PeerSessionRegistry::new(transport_handle.clone(), our_alias);

    let result = match cli.command {
        Command::Discover { timeout_ms } => {
            run_discover(&connection, timeout_ms).await
        }
        Command::Raw { duration_ms, no_probe, with_payload } => {
            let payload = if with_payload {
                our_node_id.as_bytes().to_vec()
            } else {
                vec![]
            };
            run_raw(&transport_handle, our_alias, duration_ms, !no_probe, payload).await
        }
        Command::Cdi {
            node,
            iterations,
            post_ack_delay_ms,
            timeout_ms,
            max_retries,
            discover_timeout_ms,
            json,
        } => {
            let target = NodeID::from_hex_string(&node)
                .map_err(|e| format!("invalid --node: {}", e))?;
            run_cdi(
                &connection,
                &registry,
                target,
                iterations,
                MemoryReadConfig {
                    timeout_ms,
                    max_retries,
                    post_ack_delay_ms,
                },
                discover_timeout_ms,
                cli.session_settle_ms,
                json,
            )
            .await
        }
    };

    // Shut down cleanly so the serial port is released.
    registry.shutdown().await;

    result
}

// ─── Discover ───────────────────────────────────────────────────────────────

async fn run_discover(
    connection: &Arc<Mutex<LccConnection>>,
    timeout_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[cdi-probe] Discovering nodes ({}ms window)...", timeout_ms);
    let start = Instant::now();
    let nodes = {
        let mut conn = connection.lock().await;
        conn.discover_nodes(timeout_ms).await?
    };
    let elapsed = start.elapsed();

    println!("Discovered {} node(s) in {}ms:", nodes.len(), elapsed.as_millis());
    println!("{:<20} {:<8}", "NodeID", "Alias");
    println!("{:<20} {:<8}", "------", "-----");
    for n in &nodes {
        println!("{:<20} 0x{:03X}", n.node_id.to_hex_string(), n.alias.value());
    }

    Ok(())
}

// ─── Raw frame dump ─────────────────────────────────────────────────────────

async fn run_raw(
    transport_handle: &lcc_rs::TransportHandle,
    our_alias: u16,
    duration_ms: u64,
    probe: bool,
    probe_payload: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rx = transport_handle.subscribe_all();
    eprintln!(
        "[cdi-probe] Raw mode: dumping all inbound/outbound frames for {}ms{}.",
        duration_ms,
        if probe { " after sending VerifyNodeGlobal" } else { "" }
    );

    if probe {
        let verify = GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, our_alias, probe_payload.clone())?;
        transport_handle.send(&verify).await?;
        eprintln!(
            "[cdi-probe] Sent VerifyNodeGlobal from alias 0x{:03X} (payload: {} byte(s))",
            our_alias,
            probe_payload.len()
        );
    }

    let deadline = Instant::now() + Duration::from_millis(duration_ms);
    let mut count = 0usize;
    println!(
        "{:>6}  {:<10}  {:<24}  {:<8}  {}",
        "t_ms", "header", "mti", "alias", "data"
    );
    let start = Instant::now();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(msg)) => {
                count += 1;
                let t_ms = start.elapsed().as_millis();
                let (mti_str, alias_str) = match msg.frame.get_mti() {
                    Ok((mti, alias)) => (format!("{:?}", mti), format!("0x{:03X}", alias)),
                    Err(_) => ("<unparsed>".into(), "-".into()),
                };
                let data_hex: String = msg
                    .frame
                    .data
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!(
                    "{:>6}  0x{:08X}  {:<24}  {:<8}  {}",
                    t_ms, msg.frame.header, mti_str, alias_str, data_hex
                );
            }
            Ok(Err(_)) => continue, // channel lagged
            Err(_) => break,        // deadline
        }
    }
    eprintln!("[cdi-probe] Raw mode: captured {} frame(s) in {}ms.", count, start.elapsed().as_millis());
    Ok(())
}

// ─── CDI ────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn run_cdi(
    connection: &Arc<Mutex<LccConnection>>,
    registry: &Arc<PeerSessionRegistry>,
    target: NodeID,
    iterations: usize,
    config: MemoryReadConfig,
    discover_timeout_ms: u64,
    session_settle_ms: u64,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: discover so the registry's spawn-watcher observes the target.
    eprintln!(
        "[cdi-probe] Discovering nodes ({}ms window) to prime the session registry...",
        discover_timeout_ms
    );
    let nodes = {
        let mut conn = connection.lock().await;
        conn.discover_nodes(discover_timeout_ms).await?
    };
    let saw_target = nodes.iter().any(|n| n.node_id == target);
    if !saw_target {
        eprintln!(
            "[cdi-probe] WARNING: target node {} not seen in discovery. \
             Continuing anyway — session may not spawn.",
            target.to_hex_string()
        );
    } else {
        eprintln!(
            "[cdi-probe] Target {} present in discovery ({} node(s) total).",
            target.to_hex_string(),
            nodes.len()
        );
    }

    // Step 2: let the spawn-watcher catch up. The registry watches the same
    // broadcast that discover_nodes drained, but its own subscription runs
    // on a separate task and may still be processing.
    tokio::time::sleep(Duration::from_millis(session_settle_ms)).await;

    // Step 3: fetch the session handle.
    let handle = match registry.get(target).await {
        Some(h) => h,
        None => {
            return Err(format!(
                "no peer session for {} — the registry hasn't spawned one \
                 (target may not be on the bus, or session_settle_ms is too short)",
                target.to_hex_string()
            )
            .into());
        }
    };

    // Step 4: run N downloads.
    eprintln!(
        "[cdi-probe] Running {} CDI download iteration(s) against {} \
         (post_ack_delay_ms={}, timeout_ms={}, max_retries={})",
        iterations,
        target.to_hex_string(),
        config.post_ack_delay_ms,
        config.timeout_ms,
        config.max_retries,
    );

    let mut records = Vec::with_capacity(iterations);
    let mut successes: Vec<u64> = Vec::new(); // total_duration_ms per success
    let mut total_retries_across_runs = 0usize;
    let mut failure_count = 0usize;

    if !json {
        println!(
            "{:>3}  {:<8}  {:>8}  {:>7}  {:>8}  {:>6}  {:>6}  {:>6}  {:>6}",
            "#", "status", "total_ms", "chunks", "bytes", "min", "mean", "p95", "max"
        );
    }

    for i in 1..=iterations {
        let iter_start = Instant::now();
        let result = handle.download_cdi(config.clone()).await;
        let outcome = match result {
            Ok(completion) => {
                let stats = &completion.stats;
                successes.push(stats.total_duration_ms);
                total_retries_across_runs += stats.total_retries;
                let (min_c, mean_c, p95_c, max_c) = chunk_stats(&stats.chunk_durations_ms);
                if !json {
                    println!(
                        "{:>3}  {:<8}  {:>8}  {:>7}  {:>8}  {:>6}  {:>6}  {:>6}  {:>6}",
                        i,
                        "ok",
                        stats.total_duration_ms,
                        stats.chunks,
                        stats.total_bytes,
                        min_c,
                        mean_c,
                        p95_c,
                        max_c
                    );
                }
                IterationOutcome::Ok {
                    total_bytes: stats.total_bytes,
                    chunks: stats.chunks,
                    total_duration_ms: stats.total_duration_ms,
                    total_retries: stats.total_retries,
                    chunk_min_ms: min_c,
                    chunk_max_ms: max_c,
                    chunk_mean_ms: mean_c,
                    chunk_p95_ms: p95_c,
                }
            }
            Err(e) => {
                failure_count += 1;
                let elapsed_ms = iter_start.elapsed().as_millis();
                let kind = error_kind_of(&e);
                if !json {
                    println!(
                        "{:>3}  {:<8}  {:>8}  {:>7}  {:>8}  {:<}",
                        i,
                        "FAIL",
                        elapsed_ms,
                        "-",
                        "-",
                        format!("{}: {}", kind, e),
                    );
                }
                IterationOutcome::Err {
                    error_kind: kind,
                    detail: e.to_string(),
                    elapsed_ms,
                }
            }
        };
        records.push(IterationRecord { iteration: i, outcome });
    }

    // Step 5: summary.
    let summary = SummaryRecord {
        iterations,
        successes: successes.len(),
        failures: failure_count,
        total_duration_ms_min: min_u64(&successes),
        total_duration_ms_median: median_u64(&mut successes.clone()),
        total_duration_ms_mean: mean_u64(&successes),
        total_duration_ms_p95: p95_u64(&mut successes.clone()),
        total_duration_ms_max: max_u64(&successes),
        total_retries_across_runs,
        post_ack_delay_ms: config.post_ack_delay_ms,
        timeout_ms: config.timeout_ms,
    };

    if json {
        for rec in &records {
            println!("{}", serde_json::to_string(rec)?);
        }
        println!("{}", serde_json::to_string(&summary)?);
    } else {
        println!();
        println!("── Summary ─────────────────────────────────────────");
        println!("  iterations              : {}", summary.iterations);
        println!(
            "  successes               : {} ({:.1}%)",
            summary.successes,
            if summary.iterations == 0 {
                0.0
            } else {
                100.0 * summary.successes as f64 / summary.iterations as f64
            }
        );
        println!("  failures                : {}", summary.failures);
        println!("  post_ack_delay_ms       : {}", summary.post_ack_delay_ms);
        println!("  timeout_ms (per chunk)  : {}", summary.timeout_ms);
        println!("  total DR retries        : {}", summary.total_retries_across_runs);
        if !successes.is_empty() {
            println!(
                "  total_duration_ms       : min={} median={} mean={} p95={} max={}",
                summary.total_duration_ms_min.unwrap_or(0),
                summary.total_duration_ms_median.unwrap_or(0),
                summary.total_duration_ms_mean.unwrap_or(0),
                summary.total_duration_ms_p95.unwrap_or(0),
                summary.total_duration_ms_max.unwrap_or(0),
            );
        }
    }

    if failure_count > 0 {
        std::process::exit(1);
    }
    Ok(())
}

// ─── Stats helpers ──────────────────────────────────────────────────────────

fn chunk_stats(chunks: &[u32]) -> (u32, u32, u32, u32) {
    if chunks.is_empty() {
        return (0, 0, 0, 0);
    }
    let min = *chunks.iter().min().unwrap();
    let max = *chunks.iter().max().unwrap();
    let sum: u64 = chunks.iter().map(|v| *v as u64).sum();
    let mean = (sum / chunks.len() as u64) as u32;
    let mut sorted = chunks.to_vec();
    sorted.sort_unstable();
    let p95_idx = ((sorted.len() as f64) * 0.95).ceil() as usize;
    let p95 = sorted[p95_idx.saturating_sub(1).min(sorted.len() - 1)];
    (min, mean, p95, max)
}

fn min_u64(v: &[u64]) -> Option<u64> {
    v.iter().copied().min()
}
fn max_u64(v: &[u64]) -> Option<u64> {
    v.iter().copied().max()
}
fn mean_u64(v: &[u64]) -> Option<u64> {
    if v.is_empty() {
        None
    } else {
        Some(v.iter().sum::<u64>() / v.len() as u64)
    }
}
fn median_u64(v: &mut Vec<u64>) -> Option<u64> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[v.len() / 2])
}
fn p95_u64(v: &mut Vec<u64>) -> Option<u64> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    let idx = ((v.len() as f64) * 0.95).ceil() as usize;
    Some(v[idx.saturating_sub(1).min(v.len() - 1)])
}

fn error_kind_of(e: &PeerError) -> String {
    match e {
        PeerError::Timeout { .. } => "timeout".into(),
        PeerError::TransportUnhealthy { .. } => "transport-unhealthy".into(),
        PeerError::Cancelled { .. } => "cancelled".into(),
        PeerError::Rejected { .. } => "rejected".into(),
        PeerError::PeerReinitialised => "peer-reinitialised".into(),
        PeerError::AliasChanged { .. } => "alias-changed".into(),
        PeerError::Protocol(_) => "protocol".into(),
        PeerError::NotConnected => "not-connected".into(),
        _ => "other".into(),
    }
}

