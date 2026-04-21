use anyhow::Result;
use clap::Parser;
use heartbeat_iot::common::*;
use tracing::info;

#[derive(Parser)]
#[command(name = "sensor", about = "Heartbeat-IoT — Sensor node")]
struct Args {
    #[arg(long, env = "SENSOR_ID", default_value = "sensor-1")]
    sensor_id: String,

    /// HTTP address of the local edge node
    #[arg(long, env = "EDGE_ADDR", default_value = "http://localhost:8080")]
    edge_addr: String,

    /// Publishing interval in milliseconds
    #[arg(long, env = "PUBLISH_INTERVAL_MS", default_value = "500")]
    publish_interval_ms: u64,

    /// Minimum temperature value
    #[arg(long, env = "TEMP_MIN", default_value = "15.0")]
    temp_min: f64,

    /// Maximum temperature value
    #[arg(long, env = "TEMP_MAX", default_value = "40.0")]
    temp_max: f64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Synthetic temperature with simple noise: oscillates + pseudo-random spike
fn generate_value(temp_min: f64, temp_max: f64, sequence: u64) -> f64 {
    let base = (temp_min + temp_max) / 2.0;
    let amplitude = (temp_max - temp_min) / 2.0;
    // Slow sine wave over time
    let t = (sequence as f64) * 0.05;
    let sine = t.sin() * amplitude * 0.6;
    // Small pseudo-random noise using sequence
    let noise = ((sequence.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 33) as f64
        / u32::MAX as f64
        - 0.5)
        * amplitude
        * 0.2;
    (base + sine + noise).clamp(temp_min - 2.0, temp_max + 2.0)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args = Args::parse();

    info!(
        sensor_id = %args.sensor_id,
        edge_addr = %args.edge_addr,
        interval_ms = args.publish_interval_ms,
        "Sensor starting"
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let reading_url = format!("{}/reading", args.edge_addr);
    let heartbeat_url = format!("{}/heartbeat", args.edge_addr);

    let mut sequence: u64 = 0;
    let mut heartbeat_counter: u64 = 0;

    loop {
        sequence += 1;
        heartbeat_counter += 1;

        let value = generate_value(args.temp_min, args.temp_max, sequence);
        let reading = SensorReading {
            sensor_id: args.sensor_id.clone(),
            timestamp_ms: now_ms(),
            value,
            unit: "celsius".to_string(),
            sequence,
        };

        info!(
            seq = sequence,
            value = format!("{:.2}°C", value),
            "Publishing reading"
        );

        match client.post(&reading_url).json(&reading).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!(seq = sequence, "Reading accepted by edge");
            }
            Ok(resp) => {
                tracing::warn!(status = %resp.status(), "Edge returned non-2xx");
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to reach edge — will retry next interval");
            }
        }

        // Send heartbeat every 10 publishes (~5s at 500ms interval)
        if heartbeat_counter % 10 == 0 {
            let hb = Heartbeat {
                node_id: args.sensor_id.clone(),
                role: "sensor".to_string(),
                timestamp_ms: now_ms(),
            };
            let _ = client.post(&heartbeat_url).json(&hb).send().await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(args.publish_interval_ms)).await;
    }
}
