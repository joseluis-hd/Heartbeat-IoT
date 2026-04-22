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
RUN cargo build --release --bin coordinator

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/coordinator /app/coordinator

ENV RUST_LOG=info
ENV BIND_ADDR=0.0.0.0:9000
ENV ANOMALY_THRESHOLD=38.0

EXPOSE 9000

HEALTHCHECK --interval=15s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9000/health || exit 1

CMD ["./coordinator"]
