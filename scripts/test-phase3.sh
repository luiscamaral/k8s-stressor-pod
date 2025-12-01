#!/bin/bash
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-11-28

set -e

BASE_URL="${1:-http://localhost:8080}"

echo "=== Phase 3 Integration Tests ==="
echo "Target: $BASE_URL"
echo ""

# Helper function to check metric exists
check_metric() {
    local metric_name="$1"
    local metrics="$2"
    if echo "$metrics" | grep -q "^$metric_name"; then
        return 0
    else
        return 1
    fi
}

# Test 1: Prometheus Metrics Format
echo "Test 1: Prometheus Metrics Format"
echo -n "  Content-Type header... "
CT=$(curl -sI "$BASE_URL/metrics" | grep -i "content-type" | tr -d '\r')
if echo "$CT" | grep -q "text/plain"; then
    echo "✓"
else
    echo "✗ ($CT)"
    exit 1
fi
echo ""

# Test 2: All Required Metrics Present
echo "Test 2: All Required Metrics Present"
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
    echo -n "  $metric... "
    if check_metric "$metric" "$METRICS"; then
        echo "✓"
    else
        echo "✗ NOT FOUND"
        ALL_FOUND=false
    fi
done

if [ "$ALL_FOUND" = false ]; then
    echo "✗ Some metrics missing"
    exit 1
fi
echo ""

# Test 3: Metrics Have HELP and TYPE
echo "Test 3: Metrics Have HELP and TYPE Headers"
echo -n "  HELP headers present... "
HELP_COUNT=$(echo "$METRICS" | grep -c "^# HELP" || true)
if [ "$HELP_COUNT" -ge 10 ]; then
    echo "✓ ($HELP_COUNT found)"
else
    echo "✗ (only $HELP_COUNT found)"
    exit 1
fi

echo -n "  TYPE headers present... "
TYPE_COUNT=$(echo "$METRICS" | grep -c "^# TYPE" || true)
if [ "$TYPE_COUNT" -ge 10 ]; then
    echo "✓ ($TYPE_COUNT found)"
else
    echo "✗ (only $TYPE_COUNT found)"
    exit 1
fi
echo ""

# Test 4: Idle State Metrics
echo "Test 4: Idle State Metrics"
curl -sf -X PUT "$BASE_URL/stop" > /dev/null
sleep 1
METRICS=$(curl -sf "$BASE_URL/metrics")

echo -n "  stressor_mode = 0 (idle)... "
MODE=$(echo "$METRICS" | grep "^stressor_mode " | awk '{print $2}')
if [ "$MODE" = "0" ]; then
    echo "✓"
else
    echo "✗ (got: $MODE)"
    exit 1
fi

echo -n "  stressor_is_active = 0... "
ACTIVE=$(echo "$METRICS" | grep "^stressor_is_active " | awk '{print $2}')
if [ "$ACTIVE" = "0" ]; then
    echo "✓"
else
    echo "✗ (got: $ACTIVE)"
    exit 1
fi
echo ""

# Test 5: CPU Stressor Metrics Update
echo "Test 5: CPU Stressor Metrics Update"
echo -n "  Starting CPU stressor... "
curl -sf -X PUT "$BASE_URL/config/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":500,"start_value":100,"growth_rate":50,"midpoint_ms":30000,"interval":5}' > /dev/null
curl -sf -X PUT "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"cpu-stressor"' > /dev/null
echo "✓"

echo -n "  Waiting for metrics update... "
sleep 3
METRICS=$(curl -sf "$BASE_URL/metrics")
echo "✓"

echo -n "  stressor_mode = 1 (cpu)... "
MODE=$(echo "$METRICS" | grep "^stressor_mode " | awk '{print $2}')
if [ "$MODE" = "1" ]; then
    echo "✓"
else
    echo "✗ (got: $MODE)"
fi

echo -n "  stressor_is_active = 1... "
ACTIVE=$(echo "$METRICS" | grep "^stressor_is_active " | awk '{print $2}')
if [ "$ACTIVE" = "1" ]; then
    echo "✓"
else
    echo "✗ (got: $ACTIVE)"
fi

echo -n "  CPU threads > 0... "
THREADS=$(echo "$METRICS" | grep "^stressor_cpu_active_threads " | awk '{print $2}')
if [ "$THREADS" -gt "0" ]; then
    echo "✓ ($THREADS threads)"
else
    echo "✗ (got: $THREADS)"
fi

curl -sf -X PUT "$BASE_URL/stop" > /dev/null
echo ""

# Test 6: Memory Stressor Metrics Update
echo "Test 6: Memory Stressor Metrics Update"
echo -n "  Starting memory stressor... "
curl -sf -X PUT "$BASE_URL/config/memory" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","target_mb":50,"start_mb":0,"growth_rate":10,"midpoint_ms":30000,"interval":5}' > /dev/null
curl -sf -X PUT "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"memory-stressor"' > /dev/null
echo "✓"

echo -n "  Waiting for memory allocation... "
sleep 3
METRICS=$(curl -sf "$BASE_URL/metrics")
echo "✓"

echo -n "  stressor_mode = 2 (memory)... "
MODE=$(echo "$METRICS" | grep "^stressor_mode " | awk '{print $2}')
if [ "$MODE" = "2" ]; then
    echo "✓"
else
    echo "✗ (got: $MODE)"
fi

echo -n "  Memory allocated > 0... "
MEM_ALLOC=$(echo "$METRICS" | grep "^stressor_memory_allocated_bytes " | awk '{print $2}')
if [ "$MEM_ALLOC" -gt "0" ]; then
    MEM_MB=$((MEM_ALLOC / 1024 / 1024))
    echo "✓ (${MEM_MB}MB)"
else
    echo "✗ (got: $MEM_ALLOC)"
fi

curl -sf -X PUT "$BASE_URL/stop" > /dev/null
echo ""

# Test 7: Network Stressor Metrics Update
echo "Test 7: Network Stressor Metrics Update"
echo -n "  Starting network stressor... "
curl -sf -X PUT "$BASE_URL/config/network" \
    -H "Content-Type: application/json" \
    -d '{"endpoint":"http://localhost:8080/health","protocol":"http","connections":2,"midpoint_ms":5000,"interval":5}' > /dev/null
curl -sf -X PUT "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"network-stressor"' > /dev/null
echo "✓"

echo -n "  Waiting for network activity... "
sleep 3
METRICS=$(curl -sf "$BASE_URL/metrics")
echo "✓"

echo -n "  stressor_mode = 3 (network)... "
MODE=$(echo "$METRICS" | grep "^stressor_mode " | awk '{print $2}')
if [ "$MODE" = "3" ]; then
    echo "✓"
else
    echo "✗ (got: $MODE)"
fi

echo -n "  Network requests > 0... "
NET_REQ=$(echo "$METRICS" | grep "^stressor_network_requests_total " | awk '{print $2}')
if [ "$NET_REQ" -gt "0" ]; then
    echo "✓ ($NET_REQ requests)"
else
    echo "⚠ No requests yet (endpoint may be unavailable)"
fi

curl -sf -X PUT "$BASE_URL/stop" > /dev/null
echo ""

# Test 8: Config Version Counter
echo "Test 8: Config Version Counter"
METRICS1=$(curl -sf "$BASE_URL/metrics")
V1=$(echo "$METRICS1" | grep "^stressor_config_version " | awk '{print $2}')

curl -sf -X PUT "$BASE_URL/config/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"burst","max_value":1000,"start_value":0,"growth_rate":100,"midpoint_ms":5000,"interval":5}' > /dev/null

METRICS2=$(curl -sf "$BASE_URL/metrics")
V2=$(echo "$METRICS2" | grep "^stressor_config_version " | awk '{print $2}')

echo -n "  Version incremented ($V1 -> $V2)... "
if [ "$V2" -gt "$V1" ]; then
    echo "✓"
else
    echo "✗"
    exit 1
fi
echo ""

# Test 9: Metrics Reset on Stop
echo "Test 9: Metrics Reset on Stop"
curl -sf -X PUT "$BASE_URL/mode" -H "Content-Type: application/json" -d '"cpu-stressor"' > /dev/null
sleep 2
curl -sf -X PUT "$BASE_URL/stop" > /dev/null
sleep 1

METRICS=$(curl -sf "$BASE_URL/metrics")

echo -n "  CPU metrics reset to 0... "
CPU_TARGET=$(echo "$METRICS" | grep "^stressor_cpu_target_millicores " | awk '{print $2}')
if [ "$CPU_TARGET" = "0" ]; then
    echo "✓"
else
    echo "✗ (got: $CPU_TARGET)"
fi

echo -n "  Memory metrics reset to 0... "
MEM_TARGET=$(echo "$METRICS" | grep "^stressor_memory_target_bytes " | awk '{print $2}')
if [ "$MEM_TARGET" = "0" ]; then
    echo "✓"
else
    echo "✗ (got: $MEM_TARGET)"
fi
echo ""

# Test 10: Application Version
echo "Test 10: Application Version"
echo -n "  Health endpoint returns version... "
HEALTH=$(curl -sf "$BASE_URL/health")
VERSION=$(echo "$HEALTH" | jq -r '.version')
if [ -n "$VERSION" ] && [ "$VERSION" != "null" ]; then
    echo "✓ (v$VERSION)"
else
    echo "✗ (no version found)"
fi
echo ""

# Test 11: Swagger UI Available
echo "Test 11: Swagger UI Available"
echo -n "  /swagger-ui/ returns HTML... "
SWAGGER=$(curl -sf "$BASE_URL/swagger-ui/" | head -1)
if echo "$SWAGGER" | grep -qi "html"; then
    echo "✓"
else
    echo "✗"
fi
echo ""

# Test 12: OpenAPI Spec Available
echo "Test 12: OpenAPI Spec Available"
echo -n "  /api-docs/openapi.json returns JSON... "
OPENAPI=$(curl -sf "$BASE_URL/api-docs/openapi.json")
if echo "$OPENAPI" | jq -e '.openapi' > /dev/null 2>&1; then
    OA_VERSION=$(echo "$OPENAPI" | jq -r '.info.version')
    echo "✓ (v$OA_VERSION)"
else
    echo "✗"
fi
echo ""

echo "=== All Phase 3 tests completed! ==="
