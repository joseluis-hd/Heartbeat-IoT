#!/bin/bash
# latencia_iot.sh — Simulate IoT network latency: delay 80ms jitter 20ms
# Usage: sudo ./latencia_iot.sh [interface]
# Validate with: iperf3 -c <peer> -t 10

IFACE=${1:-eth0}

echo "[latencia_iot] Applying delay 80ms jitter 20ms on $IFACE..."
tc qdisc del dev "$IFACE" root 2>/dev/null
tc qdisc add dev "$IFACE" root netem delay 80ms 20ms distribution normal

echo "[latencia_iot] Active rules:"
tc qdisc show dev "$IFACE"

echo ""
echo "[latencia_iot] Validate with: iperf3 -c <coordinator_ip> -t 10"
