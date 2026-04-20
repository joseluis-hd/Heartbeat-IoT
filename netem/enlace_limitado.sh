#!/bin/bash
# enlace_limitado.sh — Simulate limited bandwidth link: rate 512kbit delay 50ms
# Usage: sudo ./enlace_limitado.sh [interface]

IFACE=${1:-eth0}

echo "[enlace_limitado] Applying rate 512kbit delay 50ms on $IFACE..."
tc qdisc del dev "$IFACE" root 2>/dev/null
tc qdisc add dev "$IFACE" root netem rate 512kbit delay 50ms

echo "[enlace_limitado] Active rules:"
tc qdisc show dev "$IFACE"

echo ""
echo "[enlace_limitado] Validate with: iperf3 -c <coordinator_ip> -t 10"
