use anyhow::Result;
use axum::{extract::State, routing::{get, post}, Json, Router};
use clap::Parser;
use heartbeat_iot::common::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::info;

const WINDOW_SIZE: usize = 10;
// Heartbeat to coordinator every ~5s
const HEARTBEAT_EVERY_N_REPORTS: u32 = 5;

#[derive(Parser)]
#[command(name = "edge", about = "Heartbeat-IoT — Edge node")]
struct Args {
    #[arg(long, env = "EDGE_ID", default_value = "edge-1")]
    edge_id: String,

    /// HTTP address of the coordinator
    #[arg(long, env = "COORDINATOR_ADDR", default_value = "http://localhost:9000")]
    coordinator_addr: String,

    /// Port this edge node listens on
    #[arg(long, env = "PORT", default_value = "8080")]
    port: u16,

    /// Anomaly threshold — readings above this value trigger an alert
    #[arg(long, env = "ANOMALY_THRESHOLD", default_value = "38.0")]
    anomaly_threshold: f64,

    /// Base reconnect delay in milliseconds (doubles on each failure, exponential backoff)
    #[arg(long, env = "RECONNECT_BASE_MS", default_value = "500")]
    reconnect_base_ms: u64,
}

// ─── Shared state ─────────────────────────────────────────────────────────────

struct EdgeState {
    edge_id: String,
    coordinator_addr: String,
    anomaly_threshold: f64,
    reconnect_base_ms: u64,
    start_time: Instant,
    // Sliding window of recent values
    window: RwLock<VecDeque<f64>>,
    // Timestamp of last sensor reading received
    last_reading_ts: RwLock<u64>,
    // Total readings received
    total_readings: RwLock<u64>,
    // Total reports sent to coordinator
    reports_sent: RwLock<u32>,
    // Is coordinator reachable?
    coordinator_reachable: RwLock<bool>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── HTTP handlers ────────────────────────────────────────────────────────────

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        role: "edge".to_string(),
    })
}

/// POST /reading — called by sensor nodes
async fn receive_reading(
    State(state): State<Arc<EdgeState>>,
    Json(reading): Json<SensorReading>,
) -> Json<RegisterResponse> {
    info!(
        sensor = %reading.sensor_id,
        seq = reading.sequence,
        value = format!("{:.2}", reading.value),
        "Reading received"
    );

    // Update window
    {
        let mut window = state.window.write().await;
        window.push_back(reading.value);
        if window.len() > WINDOW_SIZE {
            window.pop_front();
        }
    }

    *state.last_reading_ts.write().await = reading.timestamp_ms;
    *state.total_readings.write().await += 1;

    // Compute moving average and detect anomaly
    let window = state.window.read().await;
    let avg = window.iter().sum::<f64>() / window.len() as f64;
    let anomaly = window.iter().any(|&v| v > state.anomaly_threshold);
    let sample_count = window.len() as u32;
    drop(window);

    if anomaly {
        tracing::warn!(avg = format!("{:.2}", avg), "Anomaly detected in window");
    }

    // Build report
    let report = EdgeReport {
        edge_id: state.edge_id.clone(),
        window_avg: avg,
        anomaly_detected: anomaly,
        sample_count,
        latency_ms: now_ms().saturating_sub(reading.timestamp_ms),
        latest_sensor_ts: reading.timestamp_ms,
    };

    // Forward to coordinator in background (non-blocking for the sensor)
    let state_clone = state.clone();
    tokio::spawn(async move {
        forward_to_coordinator(&state_clone, report).await;
    });

    Json(RegisterResponse {
        success: true,
        message: "Reading accepted".to_string(),
    })
}

/// POST /heartbeat — sensor heartbeats are acknowledged and optionally forwarded
async fn receive_heartbeat(
    State(state): State<Arc<EdgeState>>,
    Json(hb): Json<Heartbeat>,
) -> Json<RegisterResponse> {
    tracing::debug!(node = %hb.node_id, role = %hb.role, "Heartbeat from sensor");

    // Forward own heartbeat to coordinator
    let state_clone = state.clone();
    tokio::spawn(async move {
        send_heartbeat(&state_clone).await;
    });

    Json(RegisterResponse {
        success: true,
        message: "Heartbeat acknowledged".to_string(),
    })
}

// ─── Coordinator communication with exponential backoff ───────────────────────

async fn forward_to_coordinator(state: &EdgeState, report: EdgeReport) {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let url = format!("{}/report", state.coordinator_addr);
    let mut delay_ms = state.reconnect_base_ms;
    let max_delay_ms = 30_000u64;
    let max_retries = 5;

    for attempt in 1..=max_retries {
        match client.post(&url).json(&report).send().await {
            Ok(resp) if resp.status().is_success() => {
                *state.coordinator_reachable.write().await = true;
                *state.reports_sent.write().await += 1;
                tracing::debug!(attempt, "Report forwarded to coordinator");

                // Send heartbeat periodically
                let reports_sent = *state.reports_sent.read().await;
                if reports_sent % HEARTBEAT_EVERY_N_REPORTS == 0 {
                    send_heartbeat(state).await;
                }
                return;
            }
            Ok(resp) => {
                tracing::warn!(status = %resp.status(), attempt, "Coordinator non-2xx");
            }
            Err(e) => {
                *state.coordinator_reachable.write().await = false;
                tracing::warn!(
                    error = %e,
                    attempt,
                    retry_in_ms = delay_ms,
                    "Coordinator unreachable — backing off"
                );
            }
        }

        if attempt < max_retries {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            delay_ms = (delay_ms * 2).min(max_delay_ms);
        }
    }

    tracing::error!("Failed to forward report after {} attempts", max_retries);
}

async fn send_heartbeat(state: &EdgeState) {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap();

    let hb = Heartbeat {
        node_id: state.edge_id.clone(),
        role: "edge".to_string(),
        timestamp_ms: now_ms(),
    };

    let url = format!("{}/heartbeat", state.coordinator_addr);
    match client.post(&url).json(&hb).send().await {
        Ok(_) => tracing::debug!("Heartbeat sent to coordinator"),
        Err(e) => tracing::warn!(error = %e, "Heartbeat to coordinator failed"),
    }
}

async fn register_with_coordinator(state: &EdgeState, port: u16) {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let req = RegisterRequest {
        node_id: state.edge_id.clone(),
        role: "edge".to_string(),
        address: format!("http://{}:{}", state.edge_id, port),
    };

    let url = format!("{}/register", state.coordinator_addr);
    let mut delay_ms = state.reconnect_base_ms;

    loop {
        match client.post(&url).json(&req).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("Registered with coordinator at {}", state.coordinator_addr);
                *state.coordinator_reachable.write().await = true;
                return;
            }
            Ok(resp) => {
                tracing::warn!(status = %resp.status(), "Registration non-2xx — retrying");
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    retry_in_ms = delay_ms,
                    "Coordinator not reachable yet — retrying (disordered startup handled)"
                );
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        delay_ms = (delay_ms * 2).min(30_000);
    }
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args = Args::parse();

    info!(
        edge_id = %args.edge_id,
        coordinator = %args.coordinator_addr,
        port = args.port,
        threshold = args.anomaly_threshold,
        "Edge node starting"
    );

    let state = Arc::new(EdgeState {
        edge_id: args.edge_id.clone(),
        coordinator_addr: args.coordinator_addr.clone(),
        anomaly_threshold: args.anomaly_threshold,
        reconnect_base_ms: args.reconnect_base_ms,
        start_time: Instant::now(),
        window: RwLock::new(VecDeque::new()),
        last_reading_ts: RwLock::new(0),
        total_readings: RwLock::new(0),
        reports_sent: RwLock::new(0),
        coordinator_reachable: RwLock::new(false),
    });

    // Register with coordinator on startup (retries until success — handles disordered startup)
    let reg_state = state.clone();
    let reg_port = args.port;
    tokio::spawn(async move {
        register_with_coordinator(&reg_state, reg_port).await;
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/reading", post(receive_reading))
        .route("/heartbeat", post(receive_heartbeat))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Edge node listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
