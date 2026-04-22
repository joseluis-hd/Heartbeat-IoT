# Stage 1: Build
FROM rust:1.86-slim-bookworm AS builder

WORKDIR /build

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

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

COPY rust/src ./src
RUN cargo build --release --bin edge

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    iproute2 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/edge /app/edge

ENV RUST_LOG=info
ENV EDGE_ID=edge-1
ENV COORDINATOR_ADDR=http://coordinator:9000
ENV PORT=8080
ENV ANOMALY_THRESHOLD=38.0
ENV RECONNECT_BASE_MS=500

EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["./edge"]
