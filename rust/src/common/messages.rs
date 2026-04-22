use serde::{Deserialize, Serialize};

// ─── Sensor → Edge ────────────────────────────────────────────────────────────

/// Raw reading published by a sensor node to its local edge node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    /// Unique sensor identifier (e.g. "sensor-1")
    pub sensor_id: String,
    /// Unix timestamp in milliseconds at time of generation
    pub timestamp_ms: u64,
    /// Measured value (e.g. temperature in °C)
    pub value: f64,
    /// Unit of measurement (e.g. "celsius")
    pub unit: String,
    /// Monotonically increasing sequence number — used to detect lost messages
    pub sequence: u64,
}

// ─── Edge → Coordinator ───────────────────────────────────────────────────────

/// Aggregated report sent by an edge node to the coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeReport {
    /// Unique edge identifier (e.g. "edge-1")
    pub edge_id: String,
    /// Moving average over the last N samples
    pub window_avg: f64,
    /// True if any value in the window exceeded the anomaly threshold
    pub anomaly_detected: bool,
    /// Number of samples included in this report
    pub sample_count: u32,
    /// End-to-end latency estimate: coordinator_recv_ts - sensor_timestamp_ms
    pub latency_ms: u64,
    /// Timestamp of the most recent sensor reading included
    pub latest_sensor_ts: u64,
}

// ─── Coordinator status (HTTP endpoint) ──────────────────────────────────────

/// Coordinator status exposed at GET /status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordStatus {
    /// Number of edge nodes currently sending heartbeats
    pub active_edges: usize,
    /// Total sensor readings received since startup
    pub total_readings: u64,
    /// Number of anomalies detected in the last minute
    pub anomalies_last_min: u32,
    /// Coordinator uptime in seconds
    pub uptime_s: u64,
}

// ─── Heartbeat ────────────────────────────────────────────────────────────────

/// Periodic liveness signal sent by sensor and edge nodes to the coordinator.
/// Coordinator marks a node as down if no heartbeat is received within TIMEOUT seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Node identifier
    pub node_id: String,
    /// Role of the sending node: "sensor" | "edge"
    pub role: String,
    /// Unix timestamp in milliseconds
    pub timestamp_ms: u64,
}

// ─── Registration (edge → coordinator, mirrors P1 pattern) ───────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub node_id: String,
    pub role: String,
    /// HTTP address where the node can be reached (for coordinator pull-health)
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub success: bool,
    pub message: String,
}

// ─── Health (same as P1) ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub role: String,
}
