# Stage 1: Build — reused from P1 multi-stage pattern
FROM rust:1.86-slim-bookworm AS builder

WORKDIR /build

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies first (P1 technique)
COPY rust/Cargo.toml rust/Cargo.lock* ./

RUN mkdir -p src/bin src/common && \
    echo 'fn main() {}' > src/bin/sensor.rs && \
    echo 'fn main() {}' > src/bin/edge.rs && \
    echo 'fn main() {}' > src/bin/coordinator.rs && \
    echo 'pub mod common;' > src/lib.rs && \
    echo 'pub mod messages; pub use messages::*;' > src/common/mod.rs && \
    echo 'use serde::{Deserialize,Serialize}; #[derive(Serialize,Deserialize)] pub struct Stub{pub x:i32}' \
      > src/common/messages.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src target/release/deps/heartbeat_iot* \
           target/release/.fingerprint/heartbeat-iot-*

# Copy real source and compile the sensor binary
COPY rust/src ./src
RUN cargo build --release --bin sensor

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/sensor /app/sensor

ENV RUST_LOG=info
ENV SENSOR_ID=sensor-1
ENV EDGE_ADDR=http://edge:8080
ENV PUBLISH_INTERVAL_MS=500
ENV TEMP_MIN=15.0
ENV TEMP_MAX=40.0

EXPOSE 0

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD pgrep sensor || exit 1

CMD ["./sensor"]
