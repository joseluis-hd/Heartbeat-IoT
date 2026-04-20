#!/bin/bash
# baseline.sh — Remove all tc netem rules and restore clean network
# Usage: sudo ./baseline.sh [interface]
# Default interface: eth0

IFACE=${1:-eth0}

echo "[baseline] Removing all tc rules on $IFACE..."
tc qdisc del dev "$IFACE" root 2>/dev/null && echo "[baseline] Rules removed." || echo "[baseline] No rules to remove."

echo "[baseline] Current state:"
tc qdisc show dev "$IFACE"
