//! Minimal blocking serial test — bypasses tokio/mio-serial entirely.
//!
//! Uses the same synchronous I/O approach as JMRI (jSerialComm) to verify
//! whether the SPROG CDI stall is caused by overlapped I/O.

use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const BAUD: u32 = 460_800;
const PORT: &str = "COM8";

/// GridConnect frame bytes with \r\n terminator.
fn gc(frame: &str) -> Vec<u8> {
    let mut v = frame.as_bytes().to_vec();
    v.push(b'\r');
    v.push(b'\n');
    v
}

fn read_frames(port: &mut Box<dyn SerialPort>, timeout: Duration) -> Vec<String> {
    let start = Instant::now();
    let mut buf = Vec::new();
    let mut read_buf = [0u8; 256];
    let mut frames = Vec::new();
    let mut last_frame_time = Instant::now();

    while start.elapsed() < timeout {
        match port.read(&mut read_buf) {
            Ok(n) if n > 0 => {
                for &b in &read_buf[..n] {
                    buf.push(b);
                    if b == b';' {
                        if let Some(start_pos) = buf.iter().rposition(|&c| c == b':') {
                            if let Ok(s) = String::from_utf8(buf[start_pos..].to_vec()) {
                                frames.push(s);
                                last_frame_time = Instant::now();
                            }
                        }
                        buf.clear();
                    }
                }
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if !frames.is_empty() && last_frame_time.elapsed() > Duration::from_millis(50) {
                    break;
                }
            }
            Err(e) => {
                eprintln!("  read error: {}", e);
                break;
            }
        }
    }
    frames
}

/// Drain all stale datagrams from the target, ACKing each one to free the
/// target's datagram buffer. Keeps draining until no more data arrives.
fn drain_and_ack_stale(port: &mut Box<dyn SerialPort>) {
    loop {
        let frames = read_frames(port, Duration::from_millis(1000));
        if frames.is_empty() {
            break;
        }
        eprintln!("  drain: {} frame(s)", frames.len());
        for f in &frames {
            // ACK any DatagramFinal addressed to us: :X1D<dest><src>N...;
            if f.len() > 10 && &f[2..4] == "1D" {
                let src = &f[7..10];
                let ack = format!(":X19A28925N0{};", src);
                eprintln!("    ACK stale datagram final from 0x{}", src);
                let _ = port.write_all(&gc(&ack));
                let _ = port.flush();
            }
        }
    }
}

fn main() {
    eprintln!("=== Blocking Serial CDI Test (v2 — with stale drain) ===");
    eprintln!("Opening {} @ {} baud, flow=Hardware", PORT, BAUD);

    let mut port = serialport::new(PORT, BAUD)
        .data_bits(serialport::DataBits::Eight)
        .stop_bits(serialport::StopBits::One)
        .parity(serialport::Parity::None)
        .flow_control(serialport::FlowControl::Hardware)
        .timeout(Duration::from_millis(10))
        .open()
        .expect("Failed to open serial port");

    port.write_data_terminal_ready(true).unwrap_or_else(|e| eprintln!("DTR: {}", e));
    port.write_request_to_send(true).unwrap_or_else(|e| eprintln!("RTS: {}", e));

    eprintln!("Port open. Settling 200ms...");
    std::thread::sleep(Duration::from_millis(200));

    // Phase 1: Drain stale data before alias allocation
    eprintln!("Phase 1: Draining stale data...");
    drain_and_ack_stale(&mut port);

    // Phase 2: Alias allocation
    eprintln!("Phase 2: Alias allocation...");
    let alias_frames = [
        ":X17050925N;",
        ":X16101925N;",
        ":X1501A925N;",
        ":X142FE925N;",
        ":X10700925N;",
        ":X19100925N05010101A2FE;",
    ];
    for frame in &alias_frames {
        port.write_all(&gc(frame)).expect("write failed");
        port.flush().expect("flush failed");
    }

    // Drain and ACK any stale replies triggered by our alias appearing
    eprintln!("  Draining post-alias stale data...");
    drain_and_ack_stale(&mut port);

    // Phase 3: Discovery
    eprintln!("Phase 3: Discovery...");
    port.write_all(&gc(":X19490925N;")).expect("write failed");
    port.flush().expect("flush failed");
    let discovered = read_frames(&mut port, Duration::from_millis(500));
    eprintln!("  Discovered {} node(s)", discovered.len());

    // Final drain before CDI reads
    drain_and_ack_stale(&mut port);
    eprintln!("  Ready for CDI reads.");

    // Phase 4: CDI read
    let target_alias = "3AE";
    let chunks_to_read = 20;

    eprintln!("\nPhase 4: CDI read ({} chunks)...", chunks_to_read);
    let cdi_start = Instant::now();
    let mut total_bytes = 0usize;
    let mut successful_chunks = 0usize;

    for chunk_idx in 0..chunks_to_read {
        let offset = chunk_idx * 64;
        let request = format!(":X1A{}925N2043{:08X}40;", target_alias, offset);

        let chunk_start = Instant::now();
        port.write_all(&gc(&request)).expect("write failed");
        port.flush().expect("flush failed");

        let reply_frames = read_frames(&mut port, Duration::from_millis(5000));

        let mut got_ack = false;
        let mut got_reply_data = false;
        let mut data_bytes = 0usize;

        for f in &reply_frames {
            if f.contains("19A28") && f.contains("N0925") {
                got_ack = true;
            }
            if f.starts_with(":X1B925") || f.starts_with(":X1C925") || f.starts_with(":X1D925") {
                if let Some(n_pos) = f.find('N') {
                    let hex_data = &f[n_pos + 1..f.len() - 1];
                    data_bytes += hex_data.len() / 2;
                }
                if f.starts_with(":X1D925") {
                    got_reply_data = true;
                }
            }
        }

        let chunk_ms = chunk_start.elapsed().as_millis();

        if got_ack && got_reply_data {
            let ack = format!(":X19A28925N0{};", target_alias);
            port.write_all(&gc(&ack)).expect("write failed");
            port.flush().expect("flush failed");

            total_bytes += data_bytes;
            successful_chunks += 1;
            eprintln!("  chunk {:>3}: OK  {:>4}ms  {} bytes", chunk_idx, chunk_ms, data_bytes);
        } else {
            eprintln!("  chunk {:>3}: FAIL {:>4}ms  (ack={}, reply={}, {} frames)",
                chunk_idx, chunk_ms, got_ack, got_reply_data, reply_frames.len());
            for f in &reply_frames {
                eprintln!("    rx: {}", f);
            }
            break;
        }
    }

    let total_ms = cdi_start.elapsed().as_millis();
    eprintln!("\n=== Results ===");
    eprintln!("  Chunks OK: {}/{}", successful_chunks, chunks_to_read);
    eprintln!("  Total data bytes: {}", total_bytes);
    eprintln!("  Total time: {}ms", total_ms);
    eprintln!("  Avg per chunk: {}ms", if successful_chunks > 0 { total_ms / successful_chunks as u128 } else { 0 });

    if successful_chunks == chunks_to_read {
        eprintln!("  STATUS: SUCCESS");
    } else {
        eprintln!("  STATUS: FAILED at chunk {}", successful_chunks);
    }
}
