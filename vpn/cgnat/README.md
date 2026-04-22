# VPN Setup — Level 3 (CGNAT, no VPS)

## Network Situation

All team members operate behind CGNAT without access to a free VPS, making it impossible to configure a WireGuard hub with a reachable public IP.

**Declared Level: 3 — ZeroTier**

## Why ZeroTier

ZeroTier creates a virtual Layer 2 network over UDP hole-punching, allowing peers behind CGNAT to communicate directly without a publicly reachable hub. It was selected over Tailscale because it offers more control over the network topology and exposes a local API for programmatic management.

## Alternatives Evaluated

| Option | Reason Discarded |
|--------|-----------------|
| WireGuard hub-and-spoke (Level 1) | Requires public IP — unavailable due to CGNAT |
| WireGuard + VPS (Level 2) | No access to free VPS tier in the region |
| Tailscale | Less control, harder to document raw config |
| **ZeroTier** ✓ | Works behind CGNAT, open source, self-hostable controller |

## ZeroTier Network Configuration

```
Network ID:   b103a835d23b5a80
Subnet:       10.209.51.0/24 (auto-assigned by ZeroTier)
```

| Host | ZeroTier IP | Role |
|------|-------------|------|
| Host A | 10.209.51.38  | Coordinator + Edge-1 + Sensor-1/2 |
| Host B | 10.209.51.199 | Edge-2 + Sensor-3/4 |

## Setup Instructions

```bash
# 1. Install ZeroTier on each host
curl -s https://install.zerotier.com | sudo bash

# 2. Join the shared network
sudo zerotier-cli join b103a835d23b5a80

# 3. Authorize the node in ZeroTier Central (zerotier.com/my-network)

# 4. Verify connectivity
ping 10.209.51.<peer>
sudo zerotier-cli peers
```

## Level 3 Compensations (as required by Doc A §5.1)

- [x] tc netem: minimum 3 scenarios with iperf3 before/after measurements
- [x] mTLS between coordinator and edge nodes (self-signed certs, mutual verification in Rust)
- [x] Kubernetes: edge nodes deployed as Deployment with 2+ replicas
- [x] Additional section in final report (≥1 page): technical justification, alternatives, decision rationale
