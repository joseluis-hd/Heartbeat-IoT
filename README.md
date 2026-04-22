# 💓 heartbeat-iot

**IL355 — Programación de Sistemas Avanzados | Proyecto 2**

Distributed IoT/Edge pipeline with fault tolerance, implemented in Rust over a ZeroTier VPN (Level 3 — CGNAT) with Docker, Kubernetes, tc netem network degradation simulation, and mTLS security.

---

## Architecture Overview

```
[Sensor x2] ──► [Edge Node] ──► [Coordinator]
   (host A)       (host A)         (hub)
[Sensor x2] ──► [Edge Node]
   (host B)       (host B)
```

- **Sensor**: generates synthetic readings (temperature 15–40°C) every N ms and publishes to its local edge node.
- **Edge Node**: receives sensor data, computes a 10-sample moving average, detects threshold anomalies, forwards reports to the coordinator. Reconnects automatically with exponential backoff.
- **Coordinator**: aggregates all edge reports, detects global anomalies, tracks heartbeats, exposes an HTTP status endpoint, and logs performance metrics.

Network: **ZeroTier** (Level 3 — CGNAT, no VPS available). See [`/vpn/cgnat/`](./vpn/cgnat/) for full justification.

---

## Repository Structure

```
heartbeat-iot/
├── vpn/                    # VPN configuration (sanitized, no private keys)
│   └── cgnat/              # Level 3 CGNAT justification and ZeroTier setup
├── docker/                 # Dockerfiles for each role
│   ├── sensor/
│   ├── edge/
│   └── coordinator/
├── rust/                   # Rust workspace (sensor, edge, coordinator)
│   ├── Cargo.toml
│   ├── sensor/
│   ├── edge/
│   └── coordinator/
├── netem/                  # tc netem scripts (activate/deactivate scenarios)
├── kubernetes/             # K8s manifests (required: Level 3 + Kubernetes)
├── docs/                   # PDF deliverables and architecture diagrams
├── docker-compose.yml
├── .gitignore
└── README.md
```

---

## Requirements

| Tool | Version |
|------|---------|
| Rust + Cargo | 1.77+ |
| Docker + Docker Compose | 24.x / 2.x |
| ZeroTier | 1.14+ |
| k3s / kubectl | v1.29+ |
| tc (iproute2) | any modern |
| iperf3 | 3.x |

---

## VPN Setup (ZeroTier — Level 3)

See [`/vpn/cgnat/README.md`](./vpn/cgnat/README.md) for full setup instructions.

```bash
# Install ZeroTier
curl -s https://install.zerotier.com | sudo bash

# Join the team network
sudo zerotier-cli join <NETWORK_ID>

# Verify peers
sudo zerotier-cli peers
```

---

## Build Docker Images

```bash
# Build all images
docker build -t heartbeat-sensor ./docker/sensor/
docker build -t heartbeat-edge   ./docker/edge/
docker build -t heartbeat-coord  ./docker/coordinator/
```

---

## Deploy with Docker Compose

```bash
# Copy and configure environment
cp .env.example .env
# Edit .env: set COORDINATOR_ADDR, ZEROTIER_IP, thresholds, etc.

# Start all containers
docker-compose up -d

# View logs
docker-compose logs -f coordinator
docker-compose logs -f edge
```

---

## Build and Run Rust Manually

```bash
cd rust/

# Build all binaries (release)
cargo build --release

# Run coordinator
BIND_ADDR=0.0.0.0:9000 cargo run --release --bin coordinator

# Run edge node (on peer host)
COORDINATOR_ADDR=<coord_ip>:9000 EDGE_ID=edge-1 cargo run --release --bin edge

# Run sensor
EDGE_ADDR=127.0.0.1:8080 SENSOR_ID=sensor-1 cargo run --release --bin sensor
```

---

## Network Degradation Scenarios (tc netem)

All scripts are in [`/netem/`](./netem/). Run as root on the target host.

```bash
# Check current tc state
sudo ./netem/status.sh

# Apply scenarios
sudo ./netem/baseline.sh           # No degradation (reference)
sudo ./netem/latencia_iot.sh       # delay 80ms jitter 20ms
sudo ./netem/perdida_paquetes.sh   # loss 8%
sudo ./netem/enlace_limitado.sh    # rate 512kbit delay 50ms

# Simulate edge node failure
sudo ./netem/falla_nodo.sh edge-1  # kills container by name
```

Validate each scenario with iperf3 before running the distributed system:
```bash
# On coordinator host (server)
iperf3 -s

# On peer host (client)
iperf3 -c <coordinator_ip> -t 10
```

---

## Kubernetes (Required — Level 3)

Edge nodes are deployed as a Kubernetes Deployment with 2+ replicas.

```bash
# Apply manifests
kubectl apply -f kubernetes/

# Check deployments
kubectl get pods -n heartbeat
kubectl get deployments -n heartbeat
```

See [`/kubernetes/`](./kubernetes/) for full manifests.

---

## mTLS

Mutual TLS is enforced between coordinator and edge nodes. Self-signed certificates are generated at startup. See `/rust/coordinator/src/tls.rs` and `/rust/edge/src/tls.rs`.

---

## Performance Metrics

The coordinator exposes a status endpoint:

```bash
curl http://<coordinator_ip>:9000/status
```

Metrics tracked across all tc netem scenarios:

| Metric | Description | Unit |
|--------|-------------|------|
| Throughput | Messages processed/second | msg/s |
| E2E Latency | Sensor generation → coordinator reception | ms |
| P50 / P99 Latency | Percentiles over 60s window | ms |
| Anomaly Rate | Readings exceeding threshold | % |
| Node Uptime | Per-edge uptime since start | s |
| Lost Messages | Estimated from sequence number gaps | count |

---

## Sprint Plan

| Sprint | Dates | Goal |
|--------|-------|------|
| Sprint 1 | Apr 14 – Apr 27 | ZeroTier VPN + Docker stubs + Rust prototype (hello distributed) + baseline netem |
| Sprint 2 | Apr 28 – May 20 | Full pipeline + fault tolerance + mTLS + Kubernetes + 5 netem scenarios + metrics |

---

## Team

| Member | Role Sprint 1 | Role Sprint 2 |
|--------|--------------|--------------|
| Integrante A | VPN + tc netem + heartbeat | Pipeline Rust + mTLS + metrics |
| Integrante B | Docker + message structs + docs | Fault tolerance + Kubernetes + README |

---

## Notes and Known Limitations

- ZeroTier is used due to CGNAT constraints (Level 3). See `/vpn/cgnat/` for full technical justification.
- tc netem is applied on the **physical network interface** (not `zt0`), since ZeroTier encapsulates traffic before it reaches the virtual interface. Validated with iperf3 in each scenario.
- Clock drift between VMs may affect E2E latency calculations. Relative timestamps are used where possible.
- Rust binaries are cross-compiled inside Docker multi-stage builds to keep final images minimal.

---

## Demo Video

> 📹 Link will be added before final delivery (YouTube / TikTok — 60–90s reel)

---

## License

MIT
