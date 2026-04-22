#!/bin/bash
# falla_nodo.sh — Kill a specific edge container to simulate node failure
# Usage: sudo ./falla_nodo.sh <container_name>
# Example: sudo ./falla_nodo.sh heartbeat-edge-1

CONTAINER=${1:-}

if [ -z "$CONTAINER" ]; then
  echo "Usage: $0 <container_name>"
  echo "Available edge containers:"
  docker ps --filter "name=edge" --format "  {{.Names}}"
  exit 1
fi

echo "[falla_nodo] Stopping container: $CONTAINER"
docker stop "$CONTAINER"

echo "[falla_nodo] Container stopped. Watch coordinator logs for detection (<10s expected):"
echo "  docker-compose logs -f coordinator"
