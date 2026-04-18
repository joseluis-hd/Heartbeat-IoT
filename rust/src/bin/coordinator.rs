use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use heartbeat_iot::common::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::info;

/// An edge node is considered DOWN if no heartbeat received within this many seconds.
const HEARTBEAT_TIMEOUT_S: u64 = 10;

#[derive(Parser)]
#[command(name = "coordinator", about = "Heartbeat-IoT — Coordinator node")]
struct Args {
    #[arg(long, env = "BIND_ADDR", default_value = "0.0.0.0:9000")]
    bind_addr: String,

    /// Temperature threshold for anomaly detection
    #[arg(long, env = "ANOMALY_THRESHOLD", default_value = "38.0")]
    anomaly_threshold: f64,
}

// ─── Per-edge tracking ────────────────────────────────────────────────────────

#[derive(Debug)]
struct EdgeEntry {
    node_id: String,
    last_heartbeat: Instant,
    last_report: Option<Instant>,
    total_reports: u64,
    total_readings: u64,
    anomaly_count: u64,
    /// Running sum and count for average latency
    latency_sum_ms: u64,
    latency_count: u64,
    /// Was this edge marked as down in the previous check?
    was_down: bool,
}

impl EdgeEntry {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            last_heartbeat: Instant::now(),
            last_report: None,
            total_reports: 0,
            total_readings: 0,
            anomaly_count: 0,
            latency_sum_ms: 0,
            latency_count: 0,
            was_down: false,
        }
    }

    fn is_alive(&self) -> bool {
        self.last_heartbeat.elapsed().as_secs() < HEARTBEAT_TIMEOUT_S
    }

    fn avg_latency_ms(&self) -> u64 {
        if self.latency_count == 0 { 0 } else { self.latency_sum_ms / self.latency_count }
    }
}

// ─── Global coordinator state ─────────────────────────────────────────────────

struct CoordState {
    edges: RwLock<HashMap<String, EdgeEntry>>,
    /// Total readings aggregated across all edges
    total_readings: RwLock<u64>,
    /// Anomalies detected in the last 60 seconds (approximated as ring counter reset)
    anomalies_last_min: RwLock<u32>,
    start_time: Instant,
}

impl CoordState {
    fn new() -> Self {
        Self {
            edges: RwLock::new(HashMap::new()),
            total_readings: RwLock::new(0),
            anomalies_last_min: RwLock::new(0),
            start_time: Instant::now(),
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── Heartbeat watcher task ───────────────────────────────────────────────────

/// Runs every second and logs when an edge goes down or comes back.
async fn heartbeat_watcher(state: Arc<CoordState>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let mut edges = state.edges.write().await;
        for entry in edges.values_mut() {
            let alive = entry.is_alive();
            if !alive && !entry.was_down {
                tracing::error!(
                    edge_id = %entry.node_id,
                    last_heartbeat_s = entry.last_heartbeat.elapsed().as_secs(),
                    "EDGE DOWN — no heartbeat received within {}s",
                    HEARTBEAT_TIMEOUT_S
                );
                entry.was_down = true;
            } else if alive && entry.was_down {
                info!(
                    edge_id = %entry.node_id,
                    "EDGE RECONNECTED — heartbeats resumed"
                );
                entry.was_down = false;
            }
        }
    }
}

// ─── HTTP handlers ────────────────────────────────────────────────────────────

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        role: "coordinator".to_string(),
    })
}

/// POST /register — edge nodes announce themselves on startup
async fn register(
    State(state): State<Arc<CoordState>>,
    Json(req): Json<RegisterRequest>,
) -> Json<RegisterResponse> {
    info!(node_id = %req.node_id, role = %req.role, addr = %req.address, "Node registered");
    let mut edges = state.edges.write().await;
    edges
        .entry(req.node_id.clone())
        .or_insert_with(|| EdgeEntry::new(req.node_id.clone()));

    Json(RegisterResponse {
        success: true,
        message: format!("{} registered", req.node_id),
    })
}

/// POST /heartbeat — both sensor and edge heartbeats arrive here
async fn receive_heartbeat(
    State(state): State<Arc<CoordState>>,
    Json(hb): Json<Heartbeat>,
) -> Json<RegisterResponse> {
    tracing::debug!(node = %hb.node_id, role = %hb.role, "Heartbeat received");
    let mut edges = state.edges.write().await;
    let entry = edges
        .entry(hb.node_id.clone())
        .or_insert_with(|| EdgeEntry::new(hb.node_id.clone()));
    entry.last_heartbeat = Instant::now();

    Json(RegisterResponse {
        success: true,
        message: "Heartbeat acknowledged".to_string(),
    })
}

/// POST /report — edge nodes push aggregated reports here
async fn receive_report(
    State(state): State<Arc<CoordState>>,
    Json(report): Json<EdgeReport>,
) -> Json<RegisterResponse> {
    info!(
        edge = %report.edge_id,
        avg = format!("{:.2}°C", report.window_avg),
        anomaly = report.anomaly_detected,
        latency_ms = report.latency_ms,
        samples = report.sample_count,
        "Report received"
    );

    if report.anomaly_detected {
        tracing::warn!(
            edge = %report.edge_id,
            avg = format!("{:.2}", report.window_avg),
            "ANOMALY detected in edge window"
        );
        *state.anomalies_last_min.write().await += 1;
    }

    *state.total_readings.write().await += report.sample_count as u64;

    let mut edges = state.edges.write().await;
    let entry = edges
        .entry(report.edge_id.clone())
        .or_insert_with(|| EdgeEntry::new(report.edge_id.clone()));

    entry.last_heartbeat = Instant::now(); // report counts as liveness signal
    entry.last_report = Some(Instant::now());
    entry.total_reports += 1;
    entry.total_readings += report.sample_count as u64;
    if report.anomaly_detected {
        entry.anomaly_count += 1;
    }
    entry.latency_sum_ms += report.latency_ms;
    entry.latency_count += 1;

    Json(RegisterResponse {
        success: true,
        message: "Report accepted".to_string(),
    })
}

/// GET /status — exposes CoordStatus as JSON
async fn status(State(state): State<Arc<CoordState>>) -> Json<CoordStatus> {
    let edges = state.edges.read().await;
    let active_edges = edges.values().filter(|e| e.is_alive()).count();
    let total_readings = *state.total_readings.read().await;
    let anomalies_last_min = *state.anomalies_last_min.read().await;
    let uptime_s = state.start_time.elapsed().as_secs();

    Json(CoordStatus {
        active_edges,
        total_readings,
        anomalies_last_min,
        uptime_s,
    })
}

/// GET / — simple HTML dashboard (same style as P1, adapted for IoT metrics)
async fn dashboard(State(state): State<Arc<CoordState>>) -> Result<Html<String>, StatusCode> {
    let edges = state.edges.read().await;
    let total_readings = *state.total_readings.read().await;
    let anomalies = *state.anomalies_last_min.read().await;
    let uptime_s = state.start_time.elapsed().as_secs();

    let mut edge_rows = String::new();
    let mut edge_list: Vec<&EdgeEntry> = edges.values().collect();
    edge_list.sort_by(|a, b| a.node_id.cmp(&b.node_id));

    for e in &edge_list {
        let status_label = if e.is_alive() { "online" } else { "offline" };
        let status_class = if e.is_alive() { "idle" } else { "offline" };
        let last_hb = e.last_heartbeat.elapsed().as_secs();
        edge_rows.push_str(&format!(
            "<tr><td>{}</td><td class=\"{}\">{}</td><td>{}</td><td>{}</td><td>{}ms</td><td>{}s ago</td></tr>\n",
            e.node_id, status_class, status_label,
            e.total_reports, e.anomaly_count, e.avg_latency_ms(), last_hb
        ));
    }
    if edge_rows.is_empty() {
        edge_rows = "<tr><td colspan=\"6\" style=\"color:#444\">No edge nodes registered yet</td></tr>".to_string();
    }

    let css = "body{font-family:monospace;background:#1e1e1e;color:#d4d4d4;margin:0;padding:0}\
.wrap{max-width:860px;margin:0 auto;padding:20px 24px}\
h1{color:#569cd6;margin:0 0 2px}\
.sub{color:#555;font-size:0.8em;margin-bottom:16px}\
.card{background:#252526;border-radius:6px;padding:16px;margin-bottom:16px}\
.card h2{color:#9cdcfe;font-size:0.92em;margin:0 0 12px;padding-bottom:8px;border-bottom:1px solid #2d2d2d}\
.stats{display:flex;gap:28px;margin-bottom:8px}\
.stat .v{font-size:1.9em;color:#569cd6;font-weight:bold}\
.stat .l{font-size:0.72em;color:#777}\
table{border-collapse:collapse;width:100%}\
th{background:#2d2d2d;padding:7px 10px;text-align:left;color:#9cdcfe;font-size:0.85em}\
td{padding:5px 10px;border-bottom:1px solid #222;font-size:0.88em}\
.idle{color:#4ec9b0}.offline{color:#f44747}";

    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"UTF-8\">\
<meta http-equiv=\"refresh\" content=\"3\">\
<title>Heartbeat-IoT Coordinator</title>\
<style>{css}</style></head><body><div class=\"wrap\">\
<h1>💓 Heartbeat-IoT</h1>\
<div class=\"sub\">Coordinator — IL355 Proyecto 2</div>\
<div class=\"card\"><div class=\"stats\">\
<div class=\"stat\"><div class=\"v\">{active}</div><div class=\"l\">Active Edges</div></div>\
<div class=\"stat\"><div class=\"v\">{readings}</div><div class=\"l\">Total Readings</div></div>\
<div class=\"stat\"><div class=\"v\">{anomalies}</div><div class=\"l\">Anomalies (session)</div></div>\
<div class=\"stat\"><div class=\"v\">{uptime}s</div><div class=\"l\">Uptime</div></div>\
</div></div>\
<div class=\"card\"><h2>Edge Nodes</h2>\
<table><tr><th>ID</th><th>Status</th><th>Reports</th><th>Anomalies</th><th>Avg Latency</th><th>Last HB</th></tr>\
{rows}</table></div>\
<div style=\"color:#383838;font-size:0.78em\">Auto-refresh every 3s &nbsp;|&nbsp; \
<a href=\"/status\" style=\"color:#4ec9b0\">JSON /status</a></div>\
</div></body></html>",
        css = css,
        active = edges.values().filter(|e| e.is_alive()).count(),
        readings = total_readings,
        anomalies = anomalies,
        uptime = uptime_s,
        rows = edge_rows,
    );

    Ok(Html(html))
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args = Args::parse();

    info!(bind_addr = %args.bind_addr, threshold = args.anomaly_threshold, "Coordinator starting");

    let state = Arc::new(CoordState::new());

    // Background task: watch for dead edge nodes
    tokio::spawn(heartbeat_watcher(state.clone()));

    let app = Router::new()
        .route("/", get(dashboard))
        .route("/health", get(health))
        .route("/register", post(register))
        .route("/heartbeat", post(receive_heartbeat))
        .route("/report", post(receive_report))
        .route("/status", get(status))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&args.bind_addr).await?;
    info!("Coordinator listening on {}", args.bind_addr);
    info!("Dashboard: http://localhost:9000/");

    axum::serve(listener, app).await?;
    Ok(())
}
