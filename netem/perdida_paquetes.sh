#!/bin/bash
# perdida_paquetes.sh — Simulate 8% packet loss
# Usage: sudo ./perdida_paquetes.sh [interface]

IFACE=${1:-eth0}

echo "[perdida_paquetes] Applying loss 8% on $IFACE..."
tc qdisc del dev "$IFACE" root 2>/dev/null
tc qdisc add dev "$IFACE" root netem loss 8%

echo "[perdida_paquetes] Active rules:"
tc qdisc show dev "$IFACE"

echo ""
echo "[perdida_paquetes] Validate with: iperf3 -c <coordinator_ip> -t 10 -u"
