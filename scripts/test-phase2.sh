#!/bin/bash
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-11-28

set -e

BASE_URL="${1:-http://localhost:8080}"

echo "=== Phase 2 Integration Tests ==="
echo "Target: $BASE_URL"
echo ""

# Test 1: Health check
echo -n "Test 1: Health check... "
HEALTH=$(curl -sf "$BASE_URL/health")
STATUS=$(echo "$HEALTH" | jq -r '.status')
if [ "$STATUS" = "healthy" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (got: $HEALTH)"
    exit 1
fi

# Test 2: CPU Stressor
echo "Test 2: CPU Stressor"
echo -n "  Setting CPU config... "
curl -sf -X PUT "$BASE_URL/config/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":500,"start_value":100,"growth_rate":50,"midpoint_ms":30000,"interval":5}' > /dev/null
echo "✓"

echo -n "  Setting mode to cpu-stressor... "
curl -sf -X PUT "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"cpu-stressor"' > /dev/null
echo "✓"

echo -n "  Waiting 3s for CPU stress... "
sleep 3
echo "✓"

echo -n "  Checking metrics show active... "
ACTIVE=$(curl -sf "$BASE_URL/metrics" | grep "^stressor_is_active" | awk '{print $2}')
if [ "$ACTIVE" = "1" ]; then
    echo "✓"
else
    echo "✗ (got: $ACTIVE)"
fi

echo -n "  Checking CPU threads metric... "
THREADS=$(curl -sf "$BASE_URL/metrics" | grep "^stressor_cpu_active_threads" | awk '{print $2}')
if [ "$THREADS" -gt "0" ]; then
    echo "✓ ($THREADS threads)"
else
    echo "✗ (got: $THREADS)"
fi

echo -n "  Stopping... "
curl -sf -X PUT "$BASE_URL/stop" > /dev/null
echo "✓"
echo ""

# Test 3: Memory Stressor
echo "Test 3: Memory Stressor"
echo -n "  Setting memory config (50MB)... "
curl -sf -X PUT "$BASE_URL/config/memory" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","target_mb":50,"start_mb":0,"growth_rate":10,"midpoint_ms":30000,"interval":5}' > /dev/null
echo "✓"

echo -n "  Setting mode to memory-stressor... "
curl -sf -X PUT "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"memory-stressor"' > /dev/null
echo "✓"

echo -n "  Waiting 3s for memory allocation... "
sleep 3
echo "✓"

echo -n "  Checking mode is active... "
MODE=$(curl -sf "$BASE_URL/status" | jq -r '.mode')
if [ "$MODE" = "memory-stressor" ]; then
    echo "✓"
else
    echo "✗ (got: $MODE)"
fi

echo -n "  Checking memory metrics... "
MEM_ALLOCATED=$(curl -sf "$BASE_URL/metrics" | grep "^stressor_memory_allocated_bytes" | awk '{print $2}')
if [ "$MEM_ALLOCATED" -gt "0" ]; then
    MEM_MB=$((MEM_ALLOCATED / 1024 / 1024))
    echo "✓ (${MEM_MB}MB allocated)"
else
    echo "✗ (got: $MEM_ALLOCATED)"
fi

echo -n "  Stopping... "
curl -sf -X PUT "$BASE_URL/stop" > /dev/null
echo "✓"
echo ""

# Test 4: Mode switching
echo "Test 4: Mode Switching"
echo -n "  Switching CPU -> Memory -> Idle... "
curl -sf -X PUT "$BASE_URL/mode" -H "Content-Type: application/json" -d '"cpu-stressor"' > /dev/null
sleep 1
curl -sf -X PUT "$BASE_URL/mode" -H "Content-Type: application/json" -d '"memory-stressor"' > /dev/null
sleep 1
curl -sf -X PUT "$BASE_URL/stop" > /dev/null
MODE=$(curl -sf "$BASE_URL/status" | jq -r '.mode')
if [ "$MODE" = "idle" ]; then
    echo "✓"
else
    echo "✗ (got: $MODE)"
fi
echo ""

# Test 5: Version counter
echo "Test 5: Config Version Counter"
V1=$(curl -sf "$BASE_URL/status" | jq '.config_version')
curl -sf -X PUT "$BASE_URL/config/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"burst","max_value":1000,"start_value":0,"growth_rate":100,"midpoint_ms":5000,"interval":5}' > /dev/null
V2=$(curl -sf "$BASE_URL/status" | jq '.config_version')
echo -n "  Version incremented ($V1 -> $V2)... "
if [ "$V2" -gt "$V1" ]; then
    echo "✓"
else
    echo "✗"
fi
echo ""

# Test 6: Metrics endpoint has all expected fields
echo "Test 6: Metrics Completeness"
METRICS=$(curl -sf "$BASE_URL/metrics")
EXPECTED_METRICS=(
    "stressor_mode"
    "stressor_config_version"
    "stressor_is_active"
    "stressor_cpu_target_millicores"
    "stressor_cpu_active_threads"
    "stressor_memory_target_bytes"
    "stressor_memory_allocated_bytes"
    "stressor_network_active_connections"
    "stressor_network_requests_total"
    "stressor_network_errors_total"
)
ALL_FOUND=true
for metric in "${EXPECTED_METRICS[@]}"; do
    if echo "$METRICS" | grep -q "^$metric"; then
        echo -n "  $metric... ✓"
        echo ""
    else
        echo -n "  $metric... ✗ NOT FOUND"
        echo ""
        ALL_FOUND=false
    fi
done
echo ""

# Test 7: Graceful shutdown verification (mode resets metrics)
echo "Test 7: Metrics Reset on Stop"
curl -sf -X PUT "$BASE_URL/config/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":500,"start_value":100,"growth_rate":50,"midpoint_ms":30000,"interval":5}' > /dev/null
curl -sf -X PUT "$BASE_URL/mode" -H "Content-Type: application/json" -d '"cpu-stressor"' > /dev/null
sleep 2
curl -sf -X PUT "$BASE_URL/stop" > /dev/null
sleep 1
echo -n "  CPU metrics reset to 0... "
CPU_TARGET=$(curl -sf "$BASE_URL/metrics" | grep "^stressor_cpu_target_millicores" | awk '{print $2}')
if [ "$CPU_TARGET" = "0" ]; then
    echo "✓"
else
    echo "✗ (got: $CPU_TARGET)"
fi
echo ""

# Test 8: Network Stressor (if endpoint available)
echo "Test 8: Network Stressor"
echo -n "  Setting network config... "
curl -sf -X PUT "$BASE_URL/config/network" \
    -H "Content-Type: application/json" \
    -d '{"endpoint":"http://localhost:8080/health","protocol":"http","connections":2,"midpoint_ms":5000,"interval":5}' > /dev/null
echo "✓"

echo -n "  Setting mode to network-stressor... "
curl -sf -X PUT "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"network-stressor"' > /dev/null
echo "✓"

echo -n "  Waiting 3s for network traffic... "
sleep 3
echo "✓"

echo -n "  Checking network metrics... "
NET_REQUESTS=$(curl -sf "$BASE_URL/metrics" | grep "^stressor_network_requests_total" | awk '{print $2}')
if [ "$NET_REQUESTS" -gt "0" ]; then
    echo "✓ ($NET_REQUESTS requests)"
else
    echo "⚠ No requests (endpoint may be unavailable)"
fi

echo -n "  Stopping... "
curl -sf -X PUT "$BASE_URL/stop" > /dev/null
echo "✓"
echo ""

# Test 9: Burst mode verification
echo "Test 9: Burst Mode"
echo -n "  Setting burst CPU config... "
curl -sf -X PUT "$BASE_URL/config/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"burst","max_value":800,"start_value":100,"growth_rate":100,"midpoint_ms":3000,"interval":2}' > /dev/null
echo "✓"

echo -n "  Starting burst mode... "
curl -sf -X PUT "$BASE_URL/mode" -H "Content-Type: application/json" -d '"cpu-stressor"' > /dev/null
sleep 1
BURST_TARGET=$(curl -sf "$BASE_URL/metrics" | grep "^stressor_cpu_target_millicores" | awk '{print $2}')
echo "✓ (target: ${BURST_TARGET}m)"

curl -sf -X PUT "$BASE_URL/stop" > /dev/null
echo ""

# Test 10: Status endpoint structure
echo "Test 10: Status Endpoint Structure"
echo -n "  Checking status has is_active field... "
STATUS=$(curl -sf "$BASE_URL/status")
if echo "$STATUS" | jq 'has("is_active")' | grep -q true; then
    echo "✓"
else
    echo "✗"
fi

echo -n "  Checking status does NOT have cpu_config... "
if echo "$STATUS" | jq -e '.cpu_config' > /dev/null 2>&1; then
    echo "✗ (should not have cpu_config)"
else
    echo "✓"
fi
echo ""

echo "=== All Phase 2 tests completed! ==="
