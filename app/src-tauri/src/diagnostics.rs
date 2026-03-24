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
    /// "Tcp" | "GridConnectSerial" | "SlcanSerial"
    pub adapter_type: Option<String>,
    /// host:port or serial port name
    pub connection_label: Option<String>,

    pub discovery: DiscoveryStats,
    /// key = node_id_hex
    pub cdi_downloads: HashMap<String, CdiDownloadStats>,
    /// key = node_id_hex
    pub config_reads: HashMap<String, NodeConfigReadStats>,
    pub event_role_exchange: Option<EventRoleExchangeStats>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryStats {
    pub initial_probe_at: Option<DateTime<Utc>>,
    pub initial_probe_node_count: usize,
    /// TCP only
    pub second_probe_at: Option<DateTime<Utc>>,
    /// TCP only
    pub second_probe_node_count: Option<usize>,
    pub nodes: Vec<NodeDiscoveryStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDiscoveryStat {
    pub node_id: String,
    pub snip_name: Option<String>,
    /// Milliseconds after connection established when this node was first seen.
    pub ms_after_connect: u64,
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
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfigReadStats {
    pub node_id: String,
    pub snip_name: Option<String>,
    pub total_batches: usize,
    pub successful_batches: usize,
    pub failed_batches: usize,
    pub total_elements: usize,
    pub successful_elements: usize,
    pub failed_elements: usize,
    pub total_duration_ms: u64,
    pub batch_stats: Vec<BatchReadStat>,
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
    Ok(serde_json::json!({
        "stats": stats,
        "log": log_lines,
    }))
}
