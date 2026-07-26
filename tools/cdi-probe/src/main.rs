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
    FrameEncoding, GridConnectFrame, LccConnection,
    MemoryReadConfig, NodeID, PeerError, PeerSessionRegistry, SerialFlowControl, MTI,
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
    /// Sweep a memory-config address range in fixed-size chunks via
    /// `PeerSessionHandle::read_memory`. Exercises the config-read path
    /// (`ActiveExchange::MemoryRead`) that the Bowties app actually runs
    /// most of the time (CDI XML is cached; only config values are
    /// re-read on reconnect).
    ///
    /// Reports per-chunk timing (first-frame latency, total duration,
    /// frame count) so slow-transport symptoms can be isolated from
    /// app-side batching / progress-event overhead.
    ReadSpace {
        /// Target node ID (dotted or contiguous hex).
        #[arg(long)]
        node: String,

        /// Address space (hex, e.g. `FD` or `0xFD`). Defaults to `0xFD`
        /// (configuration).
        #[arg(long, default_value = "0xFD", value_parser = parse_hex_u8)]
        space: u8,

        /// Start address (hex, e.g. `0x80`). Defaults to `0x80`, the
        /// typical first configurable address on LCC nodes.
        #[arg(long, default_value = "0x80", value_parser = parse_hex_u32)]
        start: u32,

        /// Total number of bytes to sweep from `start`.
        #[arg(long)]
        length: u32,

        /// Chunk size per read (1..=64).
        #[arg(long, default_value_t = 64)]
        chunk_size: u8,

        /// Number of full sweeps.
        #[arg(long, default_value_t = 1)]
        iterations: usize,

        /// Per-read timeout (ms).
        #[arg(long, default_value_t = 3000)]
        timeout_ms: u64,

        /// Discovery collection window before starting reads (ms).
        #[arg(long, default_value_t = 500)]
        discover_timeout_ms: u64,

        /// Emit JSON records instead of a human-readable table.
        #[arg(long)]
        json: bool,
    },
}

fn parse_hex_u8(s: &str) -> Result<u8, String> {
    let cleaned = s.trim_start_matches("0x").trim_start_matches("0X");
    u8::from_str_radix(cleaned, 16).map_err(|e| format!("invalid hex u8 '{s}': {e}"))
}

fn parse_hex_u32(s: &str) -> Result<u32, String> {
    let cleaned = s.trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(cleaned, 16).map_err(|e| format!("invalid hex u32 '{s}': {e}"))
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

// ─── Records for `read-space` JSON output ──────────────────────────────────

/// One completed chunk read within a `read-space` sweep.
#[derive(Serialize)]
struct ReadSpaceChunkRecord {
    iteration: usize,
    chunk_index: usize,
    address: u32,
    size: u8,
    #[serde(flatten)]
    outcome: ReadSpaceChunkOutcome,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
enum ReadSpaceChunkOutcome {
    Ok {
        first_frame_latency_ms: u64,
        total_duration_ms: u64,
        frame_count: u8,
        frame_gaps_ms: Vec<u32>,
    },
    Err {
        error_kind: String,
        detail: String,
        elapsed_ms: u128,
    },
}

#[derive(Serialize)]
struct ReadSpaceIterationRecord {
    iteration: usize,
    wall_ms: u64,
    chunk_count: usize,
    success_count: usize,
    failure_count: usize,
    first_frame_latency_ms: ChunkStatsRecord,
    total_duration_ms: ChunkStatsRecord,
}

#[derive(Serialize)]
struct ChunkStatsRecord {
    min: u32,
    mean: u32,
    p95: u32,
    max: u32,
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
        cli.port, cli.baud, cli.flow, cli.encoding,
    );
    let transport = lcc_rs::GridConnectAsyncTransport::open(
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
        Command::ReadSpace {
            node,
            space,
            start,
            length,
            chunk_size,
            iterations,
            timeout_ms,
            discover_timeout_ms,
            json,
        } => {
            let target = NodeID::from_hex_string(&node)
                .map_err(|e| format!("invalid --node: {}", e))?;
            if chunk_size == 0 || chunk_size > 64 {
                return Err(format!(
                    "--chunk-size must be in 1..=64, got {}",
                    chunk_size
                )
                .into());
            }
            if length == 0 {
                return Err("--length must be > 0".into());
            }
            run_read_space(
                &connection,
                &registry,
                target,
                space,
                start,
                length,
                chunk_size,
                iterations,
                timeout_ms,
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

// ─── Read-space (config-read timing baseline) ───────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn run_read_space(
    connection: &Arc<Mutex<LccConnection>>,
    registry: &Arc<PeerSessionRegistry>,
    target: NodeID,
    space: u8,
    start: u32,
    length: u32,
    chunk_size: u8,
    iterations: usize,
    timeout_ms: u64,
    discover_timeout_ms: u64,
    session_settle_ms: u64,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: prime the registry by discovering the node.
    eprintln!(
        "[cdi-probe] Discovering nodes ({}ms window) to prime the session registry...",
        discover_timeout_ms
    );
    let nodes = {
        let mut conn = connection.lock().await;
        conn.discover_nodes(discover_timeout_ms).await?
    };
    if !nodes.iter().any(|n| n.node_id == target) {
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

    tokio::time::sleep(Duration::from_millis(session_settle_ms)).await;

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

    // Pre-compute the chunk plan (address, size) for a single sweep.
    let plan: Vec<(u32, u8)> = {
        let mut items = Vec::new();
        let end = start.saturating_add(length);
        let mut addr = start;
        while addr < end {
            let remaining = end - addr;
            let size = std::cmp::min(remaining, chunk_size as u32) as u8;
            items.push((addr, size));
            addr = addr.saturating_add(size as u32);
        }
        items
    };

    eprintln!(
        "[cdi-probe] Sweeping space 0x{:02X} @ 0x{:08X}+{} in {}-byte chunks: \
         {} chunk(s) per iteration, {} iteration(s) (timeout_ms={})",
        space,
        start,
        length,
        chunk_size,
        plan.len(),
        iterations,
        timeout_ms,
    );

    if !json {
        println!(
            "{:>3}  {:>7}  {:>10}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}",
            "#", "wall_ms", "chunks",
            "ff_min", "ff_mean", "ff_p95", "ff_max",
            "tt_min", "tt_mean", "tt_p95", "tt_max",
        );
    }

    let mut all_iter_records: Vec<ReadSpaceIterationRecord> = Vec::new();
    let mut all_chunk_records: Vec<ReadSpaceChunkRecord> = Vec::new();
    let mut wall_by_iter: Vec<u64> = Vec::new();
    let mut had_failure = false;

    for iter_idx in 1..=iterations {
        let iter_start = Instant::now();
        let mut first_frame_ms_samples: Vec<u32> = Vec::with_capacity(plan.len());
        let mut total_ms_samples: Vec<u32> = Vec::with_capacity(plan.len());
        let mut success_count = 0usize;
        let mut failure_count = 0usize;

        for (chunk_idx, &(addr, size)) in plan.iter().enumerate() {
            let call_start = Instant::now();
            let outcome = match handle.read_memory(space, addr, size, timeout_ms).await {
                Ok((_data, timing)) => {
                    success_count += 1;
                    first_frame_ms_samples.push(timing.first_frame_latency_ms as u32);
                    total_ms_samples.push(timing.total_duration_ms as u32);
                    ReadSpaceChunkOutcome::Ok {
                        first_frame_latency_ms: timing.first_frame_latency_ms,
                        total_duration_ms: timing.total_duration_ms,
                        frame_count: timing.frame_count,
                        frame_gaps_ms: timing.frame_gaps_ms,
                    }
                }
                Err(e) => {
                    failure_count += 1;
                    had_failure = true;
                    let elapsed_ms = call_start.elapsed().as_millis();
                    if !json {
                        eprintln!(
                            "[cdi-probe] iter {} chunk {} @0x{:08X}+{} FAILED after {}ms: {}: {}",
                            iter_idx,
                            chunk_idx,
                            addr,
                            size,
                            elapsed_ms,
                            error_kind_of(&e),
                            e,
                        );
                    }
                    ReadSpaceChunkOutcome::Err {
                        error_kind: error_kind_of(&e),
                        detail: e.to_string(),
                        elapsed_ms,
                    }
                }
            };
            all_chunk_records.push(ReadSpaceChunkRecord {
                iteration: iter_idx,
                chunk_index: chunk_idx,
                address: addr,
                size,
                outcome,
            });
        }

        let wall_ms = iter_start.elapsed().as_millis() as u64;
        wall_by_iter.push(wall_ms);

        let (ff_min, ff_mean, ff_p95, ff_max) = chunk_stats(&first_frame_ms_samples);
        let (tt_min, tt_mean, tt_p95, tt_max) = chunk_stats(&total_ms_samples);

        if !json {
            println!(
                "{:>3}  {:>7}  {:>10}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}",
                iter_idx,
                wall_ms,
                format!("{}/{}", success_count, plan.len()),
                ff_min, ff_mean, ff_p95, ff_max,
                tt_min, tt_mean, tt_p95, tt_max,
            );
        }

        all_iter_records.push(ReadSpaceIterationRecord {
            iteration: iter_idx,
            wall_ms,
            chunk_count: plan.len(),
            success_count,
            failure_count,
            first_frame_latency_ms: ChunkStatsRecord {
                min: ff_min, mean: ff_mean, p95: ff_p95, max: ff_max,
            },
            total_duration_ms: ChunkStatsRecord {
                min: tt_min, mean: tt_mean, p95: tt_p95, max: tt_max,
            },
        });
    }

    if json {
        for rec in &all_chunk_records {
            println!("{}", serde_json::to_string(rec)?);
        }
        for rec in &all_iter_records {
            println!("{}", serde_json::to_string(rec)?);
        }
    } else {
        println!();
        println!("── Summary ─────────────────────────────────────────");
        println!("  iterations       : {}", iterations);
        println!("  chunks/iter      : {}", plan.len());
        println!("  bytes/iter       : {}", length);
        println!("  chunk_size       : {}", chunk_size);
        println!("  timeout_ms       : {}", timeout_ms);
        if !wall_by_iter.is_empty() {
            let sum: u64 = wall_by_iter.iter().sum();
            let mean_wall = sum / wall_by_iter.len() as u64;
            let mut sorted = wall_by_iter.clone();
            sorted.sort_unstable();
            let min_wall = *sorted.first().unwrap();
            let max_wall = *sorted.last().unwrap();
            let median_wall = sorted[sorted.len() / 2];
            let per_chunk_mean = mean_wall as f64 / plan.len() as f64;
            println!(
                "  wall_ms/iter     : min={} median={} mean={} max={}",
                min_wall, median_wall, mean_wall, max_wall
            );
            println!(
                "  per-chunk mean   : {:.2} ms (wall / chunks/iter)",
                per_chunk_mean
            );
        }
    }

    if had_failure {
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

