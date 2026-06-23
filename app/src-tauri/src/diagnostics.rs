//! Diagnostic stats structs, the `bwlog!` macro, and the
//! `get_diagnostic_report` Tauri command.
//!
//! The in-memory ring buffer (`diag_log`) always accepts entries via
//! `bwlog!` regardless of build profile — `eprintln!` is redirected to the
//! Tauri log on desktop but is silently discarded on Windows GUI builds.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

// ── Ring-buffer log ────────────────────────────────────────────────────────

/// Maximum number of log lines retained in the ring buffer.
pub const LOG_RING_CAPACITY: usize = 2000;

pub type DiagLog = Arc<Mutex<VecDeque<String>>>;

pub fn new_diag_log() -> DiagLog {
    Arc::new(Mutex::new(VecDeque::with_capacity(LOG_RING_CAPACITY)))
}

// ── Frame activity ring buffer ─────────────────────────────────────────────

/// Maximum number of recent frame entries retained.
pub const FRAME_RING_CAPACITY: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameEntry {
    /// "tx" or "rx"
    pub direction: String,
    /// Milliseconds since connection established.
    pub timestamp_ms: u64,
    /// GridConnect frame string.
    pub frame: String,
}

pub type FrameRing = Arc<Mutex<VecDeque<FrameEntry>>>;

pub fn new_frame_ring() -> FrameRing {
    Arc::new(Mutex::new(VecDeque::with_capacity(FRAME_RING_CAPACITY)))
}

// ── bwlog! macro ──────────────────────────────────────────────────────────

/// Log a diagnostic message to both `eprintln!` and the in-memory ring buffer.
///
/// Usage: `bwlog!(state, "connected to {}:{}", host, port);`
///
/// The first argument must be an expression dereferencing to `AppState`
/// (e.g. `state`, `&*state`, `state.inner()`).
#[macro_export]
macro_rules! bwlog {
    ($state:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        eprintln!("{}", &msg);
        let stamped = format!("[{}] {}", chrono::Utc::now().to_rfc3339(), msg);
        if let Ok(mut buf) = $state.diag_log.try_lock() {
            if buf.len() >= $crate::diagnostics::LOG_RING_CAPACITY {
                buf.pop_front();
            }
            buf.push_back(stamped);
        }
    }};
}

// ── Diagnostic stats structs ───────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticStats {
    pub app_version: String,
    pub connected_at: Option<DateTime<Utc>>,
    /// "Tcp" | "GridConnectSerial" | "SlcanSerial" | "MergGridConnectSerial"
    pub adapter_type: Option<String>,
    /// host:port or serial port name
    pub connection_label: Option<String>,
    /// Serial baud rate (serial adapters only)
    pub baud_rate: Option<u32>,
    /// "None" | "RtsCts" | "XonXoff"
    pub flow_control: Option<String>,
    /// "GridConnect" | "SLCAN" | "MergGridConnect" | "Tcp" — derived from adapter_type
    pub frame_encoding: Option<String>,

    pub discovery: DiscoveryStats,
    /// key = node_id_hex
    pub cdi_downloads: HashMap<String, CdiDownloadStats>,
    /// key = node_id_hex
    pub config_reads: HashMap<String, NodeConfigReadStats>,
    pub event_role_exchange: Option<EventRoleExchangeStats>,
    /// Structured error events (timeouts, rejections, failures).
    pub errors: Vec<DiagError>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryStats {
    pub probes: Vec<ProbeRecord>,
    pub nodes: Vec<NodeDiscoveryStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeRecord {
    /// "connect" | "tcp-second-probe" | "user-refresh"
    pub triggered_by: String,
    pub sent_at: DateTime<Utc>,
    pub nodes_responded_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDiscoveryStat {
    pub node_id: String,
    pub snip_name: Option<String>,
    /// Milliseconds after connection established when this node was first seen.
    pub ms_after_connect: u64,
    /// Duration of the SNIP query for this node (None if not yet queried).
    pub snip_query_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiDownloadStats {
    pub node_id: String,
    pub snip_name: Option<String>,
    pub from_cache: bool,
    pub total_bytes: usize,
    pub chunks: usize,
    /// Duration of each downloaded chunk in milliseconds.
    pub chunk_durations_ms: Vec<u32>,
    /// Total retries across all chunks.
    pub retries: usize,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfigReadStats {
    pub node_id: String,
    pub snip_name: Option<String>,
    /// Full SNIP information for this node (manufacturer, model, versions, etc.)
    pub snip: Option<SnipInfo>,
    pub total_batches: usize,
    pub successful_batches: usize,
    pub failed_batches: usize,
    pub total_elements: usize,
    pub successful_elements: usize,
    pub failed_elements: usize,
    pub total_duration_ms: u64,
    pub batch_stats: Vec<BatchReadStat>,
}

/// SNIP (Simple Node Information Protocol) data captured at the time of config read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnipInfo {
    pub manufacturer: String,
    pub model: String,
    pub hardware_version: String,
    pub software_version: String,
    pub user_name: String,
    pub user_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchReadStat {
    pub address_space: u8,
    pub address: u32,
    pub byte_count: u8,
    pub success: bool,
    /// Timeout / protocol error / channel lag description (None on success).
    pub error: Option<String>,
    /// None on failure before first frame.
    pub first_frame_latency_ms: Option<u64>,
    /// Empty for single-frame datagrams.
    pub frame_gaps_ms: Vec<u32>,
    pub frame_count: Option<u8>,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRoleExchangeStats {
    pub started_at: DateTime<Utc>,
    pub nodes_queried: usize,
    pub events_sent: usize,
    pub responses_received: usize,
    pub duration_ms: u64,
}

/// A structured error event for diagnostic reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagError {
    pub at: DateTime<Utc>,
    /// Phase where the error occurred: "cdi-download", "config-read", "snip-query", "discovery".
    pub phase: String,
    pub node_id: Option<String>,
    /// "timeout", "rejection", "protocol-error", "cancelled".
    pub error_type: String,
    pub detail: String,
}

// ── AppState field type alias ──────────────────────────────────────────────

pub type DiagStats = Arc<RwLock<DiagnosticStats>>;

pub fn new_diag_stats() -> DiagStats {
    Arc::new(RwLock::new(DiagnosticStats {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        ..Default::default()
    }))
}

// ── get_diagnostic_report Tauri command ───────────────────────────────────

/// Return a JSON snapshot of all diagnostic stats and the most recent log lines.
///
/// Intended to be copied to the clipboard via the "Copy Diagnostic Report"
/// menu item for filing bug reports.
#[tauri::command]
pub async fn get_diagnostic_report(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<serde_json::Value, String> {
    let stats = state.diag_stats.read().await.clone();
    let log_lines: Vec<String> = {
        let buf = state.diag_log.lock().await;
        buf.iter().rev().take(500).cloned().collect()
    };
    let recent_frames: Vec<FrameEntry> = {
        let ring = state.frame_ring.lock().await;
        ring.iter().rev().take(50).cloned().collect()
    };
    let summary = generate_summary(&stats);
    Ok(serde_json::json!({
        "stats": stats,
        "log": log_lines,
        "recentFrameActivity": recent_frames,
        "summary": summary,
    }))
}

/// Generate plain-English one-liner summaries from diagnostic stats.
fn generate_summary(stats: &DiagnosticStats) -> Vec<String> {
    let mut lines = Vec::new();

    // Discovery summary
    let node_count = stats.discovery.nodes.len();
    if node_count > 0 {
        let max_ms = stats.discovery.nodes.iter().map(|n| n.ms_after_connect).max().unwrap_or(0);
        lines.push(format!("Discovery: {} node(s) found in {}ms", node_count, max_ms));
    } else if !stats.discovery.probes.is_empty() {
        lines.push("Discovery: probes sent but no nodes found".to_string());
    }

    // CDI summary
    let cached = stats.cdi_downloads.values().filter(|d| d.from_cache).count();
    let downloaded = stats.cdi_downloads.values().filter(|d| !d.from_cache).count();
    let total_bytes: usize = stats.cdi_downloads.values().filter(|d| !d.from_cache).map(|d| d.total_bytes).sum();
    if downloaded > 0 || cached > 0 {
        let mut parts = Vec::new();
        if downloaded > 0 {
            parts.push(format!("{} downloaded ({} bytes)", downloaded, total_bytes));
        }
        if cached > 0 {
            parts.push(format!("{} from cache", cached));
        }
        lines.push(format!("CDI: {}", parts.join(", ")));
    }

    // Config reads summary
    let config_count = stats.config_reads.len();
    if config_count > 0 {
        let total_elements: usize = stats.config_reads.values().map(|r| r.total_elements).sum();
        let failed: usize = stats.config_reads.values().map(|r| r.failed_elements).sum();
        if failed > 0 {
            lines.push(format!("Config: {} node(s) read ({} elements, {} failed)", config_count, total_elements, failed));
        } else {
            lines.push(format!("Config: {} node(s) read ({} elements)", config_count, total_elements));
        }
    }

    // Errors summary
    let error_count = stats.errors.len();
    if error_count > 0 {
        let timeouts = stats.errors.iter().filter(|e| e.error_type == "timeout").count();
        let rejections = stats.errors.iter().filter(|e| e.error_type == "rejection").count();
        let mut parts = Vec::new();
        if timeouts > 0 { parts.push(format!("{} timeout(s)", timeouts)); }
        if rejections > 0 { parts.push(format!("{} rejection(s)", rejections)); }
        let other = error_count - timeouts - rejections;
        if other > 0 { parts.push(format!("{} other", other)); }
        lines.push(format!("Errors: {}", parts.join(", ")));
    }

    lines
}
