#!/bin/bash
# Copyright (c) 2024 Luis Amaral
# Author: Luis Amaral
# Created: 2024-11-28

set -e

BASE_URL="${1:-http://localhost:8080}"

echo "=== Phase 1 Integration Tests ==="
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

# Test 2: Ready check
echo -n "Test 2: Ready check... "
READY=$(curl -sf "$BASE_URL/ready")
STATUS=$(echo "$READY" | jq -r '.status')
if [ "$STATUS" = "ready" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (got: $READY)"
    exit 1
fi

# Test 3: Status endpoint
echo -n "Test 3: Status endpoint... "
STATUS=$(curl -sf "$BASE_URL/status")
MODE=$(echo "$STATUS" | jq -r '.mode')
IS_ACTIVE=$(echo "$STATUS" | jq -r '.is_active')
if [ "$MODE" = "idle" ] && [ "$IS_ACTIVE" = "false" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (mode: $MODE, is_active: $IS_ACTIVE)"
    exit 1
fi

# Test 4: Metrics endpoint
echo -n "Test 4: Metrics endpoint... "
METRICS=$(curl -sf "$BASE_URL/metrics")
if echo "$METRICS" | grep -q "stressor_mode"; then
    echo "✓ PASS"
else
    echo "✗ FAIL"
    exit 1
fi

# Test 5: Set CPU config
echo -n "Test 5: Set CPU config... "
HTTP_CODE=$(curl -sf -o /dev/null -w "%{http_code}" \
    -X PUT "$BASE_URL/config/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":2000,"start_value":100,"growth_rate":50,"midpoint_ms":30000,"interval":10}')
if [ "$HTTP_CODE" = "200" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE)"
    exit 1
fi

# Test 6: Verify config change
echo -n "Test 6: Verify config change... "
MAX_VALUE=$(curl -sf "$BASE_URL/config/cpu" | jq '.max_value')
if [ "$MAX_VALUE" = "2000" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (max_value: $MAX_VALUE)"
    exit 1
fi

# Test 7: Set mode
echo -n "Test 7: Set mode to cpu-stressor... "
HTTP_CODE=$(curl -sf -o /dev/null -w "%{http_code}" \
    -X PUT "$BASE_URL/mode" \
    -H "Content-Type: application/json" \
    -d '"cpu-stressor"')
if [ "$HTTP_CODE" = "200" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE)"
    exit 1
fi

# Test 8: Verify mode change
echo -n "Test 8: Verify mode change... "
MODE=$(curl -sf "$BASE_URL/status" | jq -r '.mode')
IS_ACTIVE=$(curl -sf "$BASE_URL/status" | jq -r '.is_active')
if [ "$MODE" = "cpu-stressor" ] && [ "$IS_ACTIVE" = "true" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (mode: $MODE, is_active: $IS_ACTIVE)"
    exit 1
fi

# Test 9: Verify metrics show active
echo -n "Test 9: Metrics show active... "
ACTIVE=$(curl -sf "$BASE_URL/metrics" | grep "^stressor_is_active" | awk '{print $2}')
if [ "$ACTIVE" = "1" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (stressor_is_active: $ACTIVE)"
    exit 1
fi

# Test 10: Stop
echo -n "Test 10: Stop all... "
curl -sf -X PUT "$BASE_URL/stop" > /dev/null
MODE=$(curl -sf "$BASE_URL/status" | jq -r '.mode')
if [ "$MODE" = "idle" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (mode: $MODE)"
    exit 1
fi

# Test 11: Invalid config rejected (max_value = 0)
echo -n "Test 11: Invalid CPU config rejected... "
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
    -X PUT "$BASE_URL/config/cpu" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","max_value":0,"start_value":100,"growth_rate":50,"midpoint_ms":30000,"interval":10}')
if [ "$HTTP_CODE" = "400" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE, expected 400)"
    exit 1
fi

# Test 12: Invalid memory config rejected
echo -n "Test 12: Invalid memory config rejected... "
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
    -X PUT "$BASE_URL/config/memory" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","target_mb":0,"start_mb":0,"growth_rate":10,"midpoint_ms":30000,"interval":10}')
if [ "$HTTP_CODE" = "400" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE, expected 400)"
    exit 1
fi

# Test 13: Valid memory config accepted
echo -n "Test 13: Valid memory config accepted... "
HTTP_CODE=$(curl -sf -o /dev/null -w "%{http_code}" \
    -X PUT "$BASE_URL/config/memory" \
    -H "Content-Type: application/json" \
    -d '{"mode":"linear","target_mb":256,"start_mb":0,"growth_rate":10,"midpoint_ms":30000,"interval":10}')
if [ "$HTTP_CODE" = "200" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE)"
    exit 1
fi

# Test 14: Valid network config accepted
echo -n "Test 14: Valid network config accepted... "
HTTP_CODE=$(curl -sf -o /dev/null -w "%{http_code}" \
    -X PUT "$BASE_URL/config/network" \
    -H "Content-Type: application/json" \
    -d '{"endpoint":"http://example.com","protocol":"http","connections":5,"midpoint_ms":30000,"interval":5}')
if [ "$HTTP_CODE" = "200" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE)"
    exit 1
fi

# Test 15: Invalid network protocol rejected
echo -n "Test 15: Invalid network protocol rejected... "
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
    -X PUT "$BASE_URL/config/network" \
    -H "Content-Type: application/json" \
    -d '{"endpoint":"http://example.com","protocol":"ftp","connections":5,"midpoint_ms":30000,"interval":5}')
if [ "$HTTP_CODE" = "400" ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (HTTP $HTTP_CODE, expected 400)"
    exit 1
fi

# Test 16: GET /mode endpoint
echo -n "Test 16: GET /mode endpoint... "
MODE=$(curl -sf "$BASE_URL/mode")
if [ "$MODE" = '"idle"' ]; then
    echo "✓ PASS"
else
    echo "✗ FAIL (got: $MODE)"
    exit 1
fi

# Test 17: GET /config/cpu endpoint
echo -n "Test 17: GET /config/cpu endpoint... "
CPU_CONFIG=$(curl -sf "$BASE_URL/config/cpu")
if echo "$CPU_CONFIG" | jq -e '.max_value' > /dev/null; then
    echo "✓ PASS"
else
    echo "✗ FAIL (invalid response)"
    exit 1
fi

# Test 18: GET /config/memory endpoint
echo -n "Test 18: GET /config/memory endpoint... "
MEM_CONFIG=$(curl -sf "$BASE_URL/config/memory")
if echo "$MEM_CONFIG" | jq -e '.target_mb' > /dev/null; then
    echo "✓ PASS"
else
    echo "✗ FAIL (invalid response)"
    exit 1
fi

# Test 19: GET /config/network endpoint
echo -n "Test 19: GET /config/network endpoint... "
NET_CONFIG=$(curl -sf "$BASE_URL/config/network")
if echo "$NET_CONFIG" | jq -e '.endpoint' > /dev/null; then
    echo "✓ PASS"
else
    echo "✗ FAIL (invalid response)"
    exit 1
fi

echo ""
echo "=== All 19 Phase 1 tests passed! ==="
