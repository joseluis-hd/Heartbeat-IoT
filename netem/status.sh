#!/bin/bash
# status.sh — Show current tc qdisc state on all relevant interfaces

echo "========================================="
echo "  heartbeat-iot — tc netem status"
echo "========================================="

for IFACE in eth0 zt0 docker0; do
  if ip link show "$IFACE" &>/dev/null; then
    echo ""
    echo "--- $IFACE ---"
    tc qdisc show dev "$IFACE"
  fi
done

echo ""
echo "--- Running containers ---"
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" 2>/dev/null || echo "(docker not available)"
