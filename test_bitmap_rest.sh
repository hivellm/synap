#!/bin/bash
# Test script for all Bitmap REST API endpoints

BASE_URL="${1:-http://127.0.0.1:15500}"
KEY_PREFIX="test-bitmap-rest"

echo "=========================================="
echo "Testing Bitmap REST API Endpoints"
echo "Base URL: $BASE_URL"
echo "=========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

test_count=0
pass_count=0
fail_count=0

test_endpoint() {
    test_count=$((test_count + 1))
    echo -n "[$test_count] Testing $1 ... "
    
    if [ "$2" = "GET" ]; then
        response=$(curl -s -w "\n%{http_code}" "$3")
    else
        response=$(curl -s -w "\n%{http_code}" -X "$2" "$3" -H "Content-Type: application/json" -d "$4")
    fi
    
    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')
    
    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
        echo -e "${GREEN}PASS${NC} (HTTP $http_code)"
        pass_count=$((pass_count + 1))
        if [ -n "$5" ]; then
            echo "   Response: $body"
        fi
        return 0
    else
        echo -e "${RED}FAIL${NC} (HTTP $http_code)"
        echo "   Response: $body"
        fail_count=$((fail_count + 1))
        return 1
    fi
}

echo "1. SETBIT - Set bit at offset 0 to 1"
test_endpoint "POST /bitmap/$KEY_PREFIX-1/setbit" "POST" "$BASE_URL/bitmap/$KEY_PREFIX-1/setbit" '{"offset":0,"value":1}' true

echo ""
echo "2. SETBIT - Set bit at offset 7 to 1"
test_endpoint "POST /bitmap/$KEY_PREFIX-1/setbit" "POST" "$BASE_URL/bitmap/$KEY_PREFIX-1/setbit" '{"offset":7,"value":1}' true

echo ""
echo "3. GETBIT - Get bit at offset 0"
test_endpoint "GET /bitmap/$KEY_PREFIX-1/getbit/0" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-1/getbit/0" "" true

echo ""
echo "4. GETBIT - Get bit at offset 7"
test_endpoint "GET /bitmap/$KEY_PREFIX-1/getbit/7" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-1/getbit/7" "" true

echo ""
echo "5. GETBIT - Get bit at unset offset (should return 404 or 0)"
test_endpoint "GET /bitmap/$KEY_PREFIX-1/getbit/5" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-1/getbit/5" "" true

echo ""
echo "6. BITCOUNT - Count all set bits"
# First set multiple bits
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-count/setbit" -H "Content-Type: application/json" -d '{"offset":0,"value":1}' > /dev/null
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-count/setbit" -H "Content-Type: application/json" -d '{"offset":2,"value":1}' > /dev/null
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-count/setbit" -H "Content-Type: application/json" -d '{"offset":4,"value":1}' > /dev/null
test_endpoint "GET /bitmap/$KEY_PREFIX-count/bitcount" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-count/bitcount" "" true

echo ""
echo "7. BITCOUNT - Count bits in range [0, 7]"
test_endpoint "GET /bitmap/$KEY_PREFIX-count/bitcount?start=0&end=7" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-count/bitcount?start=0&end=7" "" true

echo ""
echo "8. BITPOS - Find first set bit"
# Set bit at specific position
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-pos/setbit" -H "Content-Type: application/json" -d '{"offset":10,"value":1}' > /dev/null
test_endpoint "GET /bitmap/$KEY_PREFIX-pos/bitpos?value=1" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-pos/bitpos?value=1" "" true

echo ""
echo "9. BITPOS - Find first unset bit"
test_endpoint "GET /bitmap/$KEY_PREFIX-pos/bitpos?value=0" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-pos/bitpos?value=0" "" true

echo ""
echo "10. BITOP AND - Combine two bitmaps"
# Create source bitmaps
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-op1/setbit" -H "Content-Type: application/json" -d '{"offset":0,"value":1}' > /dev/null
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-op1/setbit" -H "Content-Type: application/json" -d '{"offset":1,"value":1}' > /dev/null
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-op2/setbit" -H "Content-Type: application/json" -d '{"offset":1,"value":1}' > /dev/null
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-op2/setbit" -H "Content-Type: application/json" -d '{"offset":2,"value":1}' > /dev/null
test_endpoint "POST /bitmap/$KEY_PREFIX-result-and/bitop" "POST" "$BASE_URL/bitmap/$KEY_PREFIX-result-and/bitop" '{"operation":"AND","source_keys":["'$KEY_PREFIX'-op1","'$KEY_PREFIX'-op2"]}' true

echo ""
echo "11. BITOP OR - Combine two bitmaps"
test_endpoint "POST /bitmap/$KEY_PREFIX-result-or/bitop" "POST" "$BASE_URL/bitmap/$KEY_PREFIX-result-or/bitop" '{"operation":"OR","source_keys":["'$KEY_PREFIX'-op1","'$KEY_PREFIX'-op2"]}' true

echo ""
echo "12. BITOP XOR - Combine two bitmaps"
test_endpoint "POST /bitmap/$KEY_PREFIX-result-xor/bitop" "POST" "$BASE_URL/bitmap/$KEY_PREFIX-result-xor/bitop" '{"operation":"XOR","source_keys":["'$KEY_PREFIX'-op1","'$KEY_PREFIX'-op2"]}' true

echo ""
echo "13. BITOP NOT - Invert bitmap"
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-not-src/setbit" -H "Content-Type: application/json" -d '{"offset":0,"value":1}' > /dev/null
test_endpoint "POST /bitmap/$KEY_PREFIX-result-not/bitop" "POST" "$BASE_URL/bitmap/$KEY_PREFIX-result-not/bitop" '{"operation":"NOT","source_keys":["'$KEY_PREFIX'-not-src"]}' true

echo ""
echo "14. STATS - Get bitmap statistics"
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-stats1/setbit" -H "Content-Type: application/json" -d '{"offset":0,"value":1}' > /dev/null
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-stats2/setbit" -H "Content-Type: application/json" -d '{"offset":1,"value":1}' > /dev/null
test_endpoint "GET /bitmap/stats" "GET" "$BASE_URL/bitmap/stats" "" true

echo ""
echo "=========================================="
echo "Test Summary:"
echo "  Total: $test_count"
echo -e "  ${GREEN}Passed: $pass_count${NC}"
echo -e "  ${RED}Failed: $fail_count${NC}"
echo "=========================================="

if [ $fail_count -eq 0 ]; then
    exit 0
else
    exit 1
fi

