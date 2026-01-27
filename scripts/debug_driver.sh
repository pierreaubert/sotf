#!/bin/bash
# Debug script for SotF HAL driver

echo "Showing recent coreaudiod logs..."
log show --predicate 'process == "coreaudiod"' --last 10m --style compact | grep -i "sotf\|plugin"

echo ""
echo "Streaming new logs (Ctrl+C to stop)..."
log stream --predicate 'process == "coreaudiod"' --style compact
