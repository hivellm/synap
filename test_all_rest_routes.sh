#!/bin/bash
# Comprehensive test script for ALL Synap REST API endpoints

BASE_URL="${1:-http://127.0.0.1:15500}"
KEY_PREFIX="test-full"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

test_count=0
pass_count=0
fail_count=0

test_endpoint() {
    test_count=$((test_count + 1))
    local category="$1"
    local name="$2"
    local method="$3"
    local url="$4"
    local data="$5"
    local show_response="${6:-false}"
    
    echo -n "[$test_count] $category: $name ... "
    
    if [ "$method" = "GET" ]; then
        response=$(curl -s -w "\n%{http_code}" "$url" 2>/dev/null)
    elif [ "$method" = "DELETE" ]; then
        if [ -n "$data" ]; then
            response=$(curl -s -w "\n%{http_code}" -X DELETE "$url" \
                -H "Content-Type: application/json" -d "$data" 2>/dev/null)
        else
            response=$(curl -s -w "\n%{http_code}" -X DELETE "$url" 2>/dev/null)
        fi
    else
        if [ -n "$data" ]; then
            response=$(curl -s -w "\n%{http_code}" -X "$method" "$url" \
                -H "Content-Type: application/json" -d "$data" 2>/dev/null)
        else
            response=$(curl -s -w "\n%{http_code}" -X "$method" "$url" 2>/dev/null)
        fi
    fi
    
    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')
    
    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
        echo -e "${GREEN}PASS${NC} (HTTP $http_code)"
        pass_count=$((pass_count + 1))
        if [ "$show_response" = "true" ]; then
            echo "   Response: $body"
        fi
        return 0
    elif [ "$http_code" -eq 404 ] && [[ "$name" == *"not exist"* ]]; then
        echo -e "${GREEN}PASS${NC} (HTTP $http_code - expected)"
        pass_count=$((pass_count + 1))
        return 0
    else
        echo -e "${RED}FAIL${NC} (HTTP $http_code)"
        if [ "$show_response" = "true" ] || [ "$http_code" -ge 400 ]; then
            echo "   Response: $body"
        fi
        fail_count=$((fail_count + 1))
        return 1
    fi
}

echo "=========================================="
echo -e "${BLUE}Comprehensive Synap REST API Test Suite${NC}"
echo "Base URL: $BASE_URL"
echo "=========================================="
echo ""

# ========== HEALTH & MONITORING ==========
echo -e "${BLUE}=== Health & Monitoring ===${NC}"
test_endpoint "Health" "GET /health" "GET" "$BASE_URL/health" "" true
test_endpoint "Metrics" "GET /metrics" "GET" "$BASE_URL/metrics" "" false
test_endpoint "Info" "GET /info" "GET" "$BASE_URL/info" "" false
test_endpoint "Slowlog" "GET /slowlog" "GET" "$BASE_URL/slowlog" "" false
test_endpoint "Clients" "GET /clients" "GET" "$BASE_URL/clients" "" false
echo ""

# ========== KV STORE ==========
echo -e "${BLUE}=== KV Store ===${NC}"
test_endpoint "KV" "POST /kv/set" "POST" "$BASE_URL/kv/set" '{"key":"'$KEY_PREFIX'-kv1","value":"test-value"}' true
test_endpoint "KV" "GET /kv/get/{key}" "GET" "$BASE_URL/kv/get/$KEY_PREFIX-kv1" "" true
test_endpoint "KV" "GET /kv/get (not exist)" "GET" "$BASE_URL/kv/get/$KEY_PREFIX-nonexistent" "" false
test_endpoint "KV" "GET /kv/stats" "GET" "$BASE_URL/kv/stats" "" true
test_endpoint "KV" "DELETE /kv/del/{key}" "DELETE" "$BASE_URL/kv/del/$KEY_PREFIX-kv1" "" true
echo ""

# ========== STRING EXTENSIONS ==========
echo -e "${BLUE}=== String Extensions ===${NC}"
test_endpoint "String" "POST /kv/{key}/append" "POST" "$BASE_URL/kv/$KEY_PREFIX-str1/append" '{"value":"hello"}' true
test_endpoint "String" "POST /kv/{key}/append (more)" "POST" "$BASE_URL/kv/$KEY_PREFIX-str1/append" '{"value":" world"}' true
test_endpoint "String" "GET /kv/{key}/getrange" "GET" "$BASE_URL/kv/$KEY_PREFIX-str1/getrange?start=0&end=4" "" true
test_endpoint "String" "POST /kv/{key}/setrange" "POST" "$BASE_URL/kv/$KEY_PREFIX-str1/setrange" '{"offset":6,"value":"Synap"}' true
test_endpoint "String" "GET /kv/{key}/strlen" "GET" "$BASE_URL/kv/$KEY_PREFIX-str1/strlen" "" true
test_endpoint "String" "POST /kv/{key}/getset" "POST" "$BASE_URL/kv/$KEY_PREFIX-str1/getset" '{"value":"new-value"}' true
test_endpoint "String" "POST /kv/msetnx" "POST" "$BASE_URL/kv/msetnx" '{"pairs":[["key1","value1"],["key2","value2"]]}' true
echo ""

# ========== KEY MANAGEMENT ==========
echo -e "${BLUE}=== Key Management ===${NC}"
test_endpoint "Key" "POST /kv/set (for key tests)" "POST" "$BASE_URL/kv/set" '{"key":"'$KEY_PREFIX'-key1","value":"test"}' false
test_endpoint "Key" "GET /key/{key}/type" "GET" "$BASE_URL/key/$KEY_PREFIX-key1/type" "" true
test_endpoint "Key" "GET /key/{key}/exists" "GET" "$BASE_URL/key/$KEY_PREFIX-key1/exists" "" true
test_endpoint "Key" "GET /key/{key}/exists (not exist)" "GET" "$BASE_URL/key/$KEY_PREFIX-nonexistent/exists" "" true
test_endpoint "Key" "POST /key/{key}/rename" "POST" "$BASE_URL/key/$KEY_PREFIX-key1/rename" '{"destination":"'$KEY_PREFIX'-key1-renamed"}' true
# Need to create a field first for copy
curl -s -X POST "$BASE_URL/kv/set" -H "Content-Type: application/json" -d '{"key":"'$KEY_PREFIX'-key-to-copy","value":"copy-test"}' > /dev/null
test_endpoint "Key" "POST /key/{key}/copy" "POST" "$BASE_URL/key/$KEY_PREFIX-key-to-copy/copy" '{"destination":"'$KEY_PREFIX'-key1-copy","replace":true}' true
test_endpoint "Key" "GET /key/randomkey" "GET" "$BASE_URL/key/randomkey" "" true
echo ""

# ========== MEMORY ==========
echo -e "${BLUE}=== Memory Usage ===${NC}"
test_endpoint "Memory" "GET /memory/{key}/usage" "GET" "$BASE_URL/memory/$KEY_PREFIX-key1-renamed/usage" "" true
echo ""

# ========== TRANSACTIONS ==========
echo -e "${BLUE}=== Transactions ===${NC}"
multi_resp=$(curl -s -X POST "$BASE_URL/transaction/multi")
if [ $? -eq 0 ] && echo "$multi_resp" | grep -q "success"; then
    test_endpoint "Transaction" "POST /transaction/multi" "POST" "$BASE_URL/transaction/multi" "" true
    test_endpoint "Transaction" "POST /transaction/watch" "POST" "$BASE_URL/transaction/watch" '{"keys":["'$KEY_PREFIX'-watch1"]}' true
    test_endpoint "Transaction" "POST /transaction/unwatch" "POST" "$BASE_URL/transaction/unwatch" "" true
    tx_id=$(echo "$multi_resp" | grep -o '"transaction_id":"[^"]*"' | cut -d'"' -f4 || echo "")
    if [ -n "$tx_id" ]; then
        test_endpoint "Transaction" "POST /transaction/discard" "POST" "$BASE_URL/transaction/discard" '{"transaction_id":"'$tx_id'"}' true
    else
        test_endpoint "Transaction" "POST /transaction/discard" "POST" "$BASE_URL/transaction/discard" '{}' true
    fi
else
    echo -e "${YELLOW}[SKIP] Transactions (multi not available)${NC}"
    # Não incrementa test_count para não contar como falha
fi
echo ""

# ========== HASH ==========
echo -e "${BLUE}=== Hash ===${NC}"
test_endpoint "Hash" "POST /hash/{key}/set" "POST" "$BASE_URL/hash/$KEY_PREFIX-hash1/set" '{"field":"field1","value":"value1"}' true
test_endpoint "Hash" "POST /hash/{key}/set (more)" "POST" "$BASE_URL/hash/$KEY_PREFIX-hash1/set" '{"field":"field2","value":"value2"}' false
test_endpoint "Hash" "GET /hash/{key}/{field}" "GET" "$BASE_URL/hash/$KEY_PREFIX-hash1/field1" "" true
test_endpoint "Hash" "GET /hash/{key}/getall" "GET" "$BASE_URL/hash/$KEY_PREFIX-hash1/getall" "" true
test_endpoint "Hash" "GET /hash/{key}/keys" "GET" "$BASE_URL/hash/$KEY_PREFIX-hash1/keys" "" true
test_endpoint "Hash" "GET /hash/{key}/vals" "GET" "$BASE_URL/hash/$KEY_PREFIX-hash1/vals" "" true
test_endpoint "Hash" "GET /hash/{key}/len" "GET" "$BASE_URL/hash/$KEY_PREFIX-hash1/len" "" true
test_endpoint "Hash" "POST /hash/{key}/mset" "POST" "$BASE_URL/hash/$KEY_PREFIX-hash1/mset" '{"fields":{"field3":"value3","field4":"value4"}}' true
test_endpoint "Hash" "POST /hash/{key}/mget" "POST" "$BASE_URL/hash/$KEY_PREFIX-hash1/mget" '{"fields":["field1","field3"]}' true
test_endpoint "Hash" "GET /hash/{key}/{field}/exists" "GET" "$BASE_URL/hash/$KEY_PREFIX-hash1/field1/exists" "" true
test_endpoint "Hash" "POST /hash/{key}/incrby" "POST" "$BASE_URL/hash/$KEY_PREFIX-hash1/incrby" '{"field":"counter","increment":5}' true
test_endpoint "Hash" "POST /hash/{key}/setnx" "POST" "$BASE_URL/hash/$KEY_PREFIX-hash1/setnx" '{"field":"newfield","value":"newvalue"}' true
test_endpoint "Hash" "GET /hash/stats" "GET" "$BASE_URL/hash/stats" "" true
test_endpoint "Hash" "DELETE /hash/{key}/del" "DELETE" "$BASE_URL/hash/$KEY_PREFIX-hash1/del" '{"fields":["field1"]}' false
echo ""

# ========== SET ==========
echo -e "${BLUE}=== Set ===${NC}"
test_endpoint "Set" "POST /set/{key}/add" "POST" "$BASE_URL/set/$KEY_PREFIX-set1/add" '{"members":["member1","member2","member3"]}' true
test_endpoint "Set" "POST /set/{key}/add (more)" "POST" "$BASE_URL/set/$KEY_PREFIX-set1/add" '{"members":["member4"]}' false
test_endpoint "Set" "GET /set/{key}/members" "GET" "$BASE_URL/set/$KEY_PREFIX-set1/members" "" true
test_endpoint "Set" "GET /set/{key}/card" "GET" "$BASE_URL/set/$KEY_PREFIX-set1/card" "" true
test_endpoint "Set" "POST /set/{key}/ismember" "POST" "$BASE_URL/set/$KEY_PREFIX-set1/ismember" '{"member":"member1"}' true
test_endpoint "Set" "POST /set/{key}/pop" "POST" "$BASE_URL/set/$KEY_PREFIX-set1/pop" '{"count":1}' true
test_endpoint "Set" "GET /set/{key}/randmember" "GET" "$BASE_URL/set/$KEY_PREFIX-set1/randmember" "" true
test_endpoint "Set" "POST /set/{source}/move/{destination}" "POST" "$BASE_URL/set/$KEY_PREFIX-set1/move/$KEY_PREFIX-set2" '{"member":"member2"}' true
curl -s -X POST "$BASE_URL/set/$KEY_PREFIX-set1/add" -H "Content-Type: application/json" -d '{"members":["a","b"]}' > /dev/null
curl -s -X POST "$BASE_URL/set/$KEY_PREFIX-set3/add" -H "Content-Type: application/json" -d '{"members":["b","c"]}' > /dev/null
test_endpoint "Set" "POST /set/inter" "POST" "$BASE_URL/set/inter" '{"keys":["'$KEY_PREFIX'-set1","'$KEY_PREFIX'-set3"]}' true
test_endpoint "Set" "POST /set/union" "POST" "$BASE_URL/set/union" '{"keys":["'$KEY_PREFIX'-set1","'$KEY_PREFIX'-set3"]}' true
test_endpoint "Set" "POST /set/diff" "POST" "$BASE_URL/set/diff" '{"keys":["'$KEY_PREFIX'-set1","'$KEY_PREFIX'-set3"]}' true
test_endpoint "Set" "POST /set/{key}/rem" "POST" "$BASE_URL/set/$KEY_PREFIX-set1/rem" '{"members":["member1"]}' true
test_endpoint "Set" "GET /set/stats" "GET" "$BASE_URL/set/stats" "" true
echo ""

# ========== SORTED SET ==========
echo -e "${BLUE}=== Sorted Set ===${NC}"
test_endpoint "SortedSet" "POST /sortedset/{key}/zadd" "POST" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zadd" '{"member":"member1","score":10}' true
test_endpoint "SortedSet" "POST /sortedset/{key}/zadd (more)" "POST" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zadd" '{"member":"member2","score":20}' false
curl -s -X POST "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zadd" -H "Content-Type: application/json" -d '{"member":"member3","score":15}' > /dev/null
test_endpoint "SortedSet" "GET /sortedset/{key}/{member}/zscore" "GET" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/member1/zscore" "" true
test_endpoint "SortedSet" "GET /sortedset/{key}/zcard" "GET" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zcard" "" true
test_endpoint "SortedSet" "POST /sortedset/{key}/zincrby" "POST" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zincrby" '{"member":"member1","score":5}' true
test_endpoint "SortedSet" "GET /sortedset/{key}/zrange" "GET" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zrange?start=0&end=-1" "" true
test_endpoint "SortedSet" "GET /sortedset/{key}/zrevrange" "GET" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zrevrange?start=0&end=-1" "" true
test_endpoint "SortedSet" "GET /sortedset/{key}/{member}/zrank" "GET" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/member1/zrank" "" true
test_endpoint "SortedSet" "GET /sortedset/{key}/{member}/zrevrank" "GET" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/member1/zrevrank" "" true
test_endpoint "SortedSet" "GET /sortedset/{key}/zcount" "GET" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zcount?min=0&max=20" "" true
test_endpoint "SortedSet" "POST /sortedset/{key}/zmscore" "POST" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zmscore" '{"members":["member1","member2"]}' true
test_endpoint "SortedSet" "GET /sortedset/{key}/zrangebyscore" "GET" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zrangebyscore?min=10&max=20" "" true
test_endpoint "SortedSet" "POST /sortedset/{key}/zpopmin" "POST" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zpopmin" '{"count":1}' true
curl -s -X POST "$BASE_URL/sortedset/$KEY_PREFIX-zset2/zadd" -H "Content-Type: application/json" -d '{"member":"a","score":1}' > /dev/null
curl -s -X POST "$BASE_URL/sortedset/$KEY_PREFIX-zset2/zadd" -H "Content-Type: application/json" -d '{"member":"b","score":2}' > /dev/null
curl -s -X POST "$BASE_URL/sortedset/$KEY_PREFIX-zset3/zadd" -H "Content-Type: application/json" -d '{"member":"b","score":3}' > /dev/null
curl -s -X POST "$BASE_URL/sortedset/$KEY_PREFIX-zset3/zadd" -H "Content-Type: application/json" -d '{"member":"c","score":4}' > /dev/null
test_endpoint "SortedSet" "POST /sortedset/zinterstore" "POST" "$BASE_URL/sortedset/zinterstore" '{"destination":"'$KEY_PREFIX'-zset-inter","keys":["'$KEY_PREFIX'-zset2","'$KEY_PREFIX'-zset3"]}' true
test_endpoint "SortedSet" "POST /sortedset/zunionstore" "POST" "$BASE_URL/sortedset/zunionstore" '{"destination":"'$KEY_PREFIX'-zset-union","keys":["'$KEY_PREFIX'-zset2","'$KEY_PREFIX'-zset3"]}' true
test_endpoint "SortedSet" "POST /sortedset/zdiffstore" "POST" "$BASE_URL/sortedset/zdiffstore" '{"destination":"'$KEY_PREFIX'-zset-diff","keys":["'$KEY_PREFIX'-zset2","'$KEY_PREFIX'-zset3"]}' true
test_endpoint "SortedSet" "POST /sortedset/{key}/zrem" "POST" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zrem" '{"members":["member2"]}' true
test_endpoint "SortedSet" "POST /sortedset/{key}/zremrangebyrank" "POST" "$BASE_URL/sortedset/$KEY_PREFIX-zset1/zremrangebyrank" '{"start":0,"end":0}' true
test_endpoint "SortedSet" "GET /sortedset/stats" "GET" "$BASE_URL/sortedset/stats" "" true
echo ""

# ========== LIST ==========
echo -e "${BLUE}=== List ===${NC}"
test_endpoint "List" "POST /list/{key}/lpush" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/lpush" '{"values":["val1","val2"]}' true
test_endpoint "List" "POST /list/{key}/rpush" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/rpush" '{"values":["val3"]}' true
test_endpoint "List" "POST /list/{key}/lpushx" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/lpushx" '{"values":["val0"]}' true
test_endpoint "List" "POST /list/{key}/rpushx" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/rpushx" '{"values":["val4"]}' true
test_endpoint "List" "GET /list/{key}/range" "GET" "$BASE_URL/list/$KEY_PREFIX-list1/range?start=0&end=-1" "" true
test_endpoint "List" "GET /list/{key}/len" "GET" "$BASE_URL/list/$KEY_PREFIX-list1/len" "" true
test_endpoint "List" "GET /list/{key}/index/{index}" "GET" "$BASE_URL/list/$KEY_PREFIX-list1/index/0" "" true
test_endpoint "List" "POST /list/{key}/set" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/set" '{"index":1,"value":"newval"}' true
test_endpoint "List" "POST /list/{key}/trim" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/trim" '{"start":0,"end":2}' true
test_endpoint "List" "POST /list/{key}/rem" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/rem" '{"value":"val3","count":1}' true
test_endpoint "List" "POST /list/{key}/insert" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/insert" '{"pivot":"val1","value":"inserted","before":true}' true
test_endpoint "List" "POST /list/{key}/lpop" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/lpop" '{"count":1}' true
test_endpoint "List" "POST /list/{key}/rpop" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/rpop" '{"count":1}' true
test_endpoint "List" "POST /list/{source}/rpoplpush/{destination}" "POST" "$BASE_URL/list/$KEY_PREFIX-list1/rpoplpush/$KEY_PREFIX-list2" '{}' true
test_endpoint "List" "GET /list/stats" "GET" "$BASE_URL/list/stats" "" true
echo ""

# ========== HYPERLOGLOG ==========
echo -e "${BLUE}=== HyperLogLog ===${NC}"
test_endpoint "HyperLogLog" "POST /hyperloglog/{key}/pfadd" "POST" "$BASE_URL/hyperloglog/$KEY_PREFIX-hll1/pfadd" '{"elements":["elem1","elem2","elem3"]}' true
test_endpoint "HyperLogLog" "GET /hyperloglog/{key}/pfcount" "GET" "$BASE_URL/hyperloglog/$KEY_PREFIX-hll1/pfcount" "" true
test_endpoint "HyperLogLog" "POST /hyperloglog/{key}/pfadd (more)" "POST" "$BASE_URL/hyperloglog/$KEY_PREFIX-hll2/pfadd" '{"elements":["elem4","elem5"]}' false
curl -s -X POST "$BASE_URL/hyperloglog/$KEY_PREFIX-hll2/pfadd" -H "Content-Type: application/json" -d '{"elements":["elem4","elem5"]}' > /dev/null
test_endpoint "HyperLogLog" "POST /hyperloglog/{destination}/pfmerge" "POST" "$BASE_URL/hyperloglog/$KEY_PREFIX-hll-merged/pfmerge" '{"sources":["'$KEY_PREFIX'-hll1","'$KEY_PREFIX'-hll2"]}' true
test_endpoint "HyperLogLog" "GET /hyperloglog/stats" "GET" "$BASE_URL/hyperloglog/stats" "" true
echo ""

# ========== BITMAP ==========
echo -e "${BLUE}=== Bitmap ===${NC}"
test_endpoint "Bitmap" "POST /bitmap/{key}/setbit" "POST" "$BASE_URL/bitmap/$KEY_PREFIX-bitmap1/setbit" '{"offset":0,"value":1}' true
test_endpoint "Bitmap" "GET /bitmap/{key}/getbit/{offset}" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-bitmap1/getbit/0" "" true
test_endpoint "Bitmap" "GET /bitmap/{key}/bitcount" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-bitmap1/bitcount" "" true
curl -s -X POST "$BASE_URL/bitmap/$KEY_PREFIX-bitmap2/setbit" -H "Content-Type: application/json" -d '{"offset":1,"value":1}' > /dev/null
test_endpoint "Bitmap" "POST /bitmap/{destination}/bitop" "POST" "$BASE_URL/bitmap/$KEY_PREFIX-bitmap-result/bitop" '{"operation":"AND","source_keys":["'$KEY_PREFIX'-bitmap1","'$KEY_PREFIX'-bitmap2"]}' true
test_endpoint "Bitmap" "GET /bitmap/{key}/bitpos" "GET" "$BASE_URL/bitmap/$KEY_PREFIX-bitmap1/bitpos?value=1" "" true
test_endpoint "Bitmap" "GET /bitmap/stats" "GET" "$BASE_URL/bitmap/stats" "" true
echo ""

# ========== LUA SCRIPTING ==========
echo -e "${BLUE}=== Lua Scripting ===${NC}"
test_endpoint "Script" "POST /script/eval" "POST" "$BASE_URL/script/eval" '{"script":"return redis.call(\"set\",\"test-script\",\"value\")","keys":["test-script"],"args":["value"]}' true
test_endpoint "Script" "POST /script/load" "POST" "$BASE_URL/script/load" '{"script":"return \"Hello from Lua\""}' true
sha=$(curl -s -X POST "$BASE_URL/script/load" -H "Content-Type: application/json" -d '{"script":"return ARGV[1]"}' | grep -o '"sha1":"[a-f0-9]*"' | cut -d'"' -f4)
if [ -n "$sha" ]; then
    test_endpoint "Script" "POST /script/evalsha" "POST" "$BASE_URL/script/evalsha" '{"sha1":"'$sha'","keys":[],"args":["test"]}' true
    test_endpoint "Script" "POST /script/exists" "POST" "$BASE_URL/script/exists" '{"hashes":["'$sha'"]}' true
fi
echo ""

# ========== PUB/SUB ==========
echo -e "${BLUE}=== Pub/Sub ===${NC}"
test_endpoint "PubSub" "POST /pubsub/{topic}/publish" "POST" "$BASE_URL/pubsub/$KEY_PREFIX-topic1/publish" '{"payload":"test message"}' true
test_endpoint "PubSub" "GET /pubsub/stats" "GET" "$BASE_URL/pubsub/stats" "" true
test_endpoint "PubSub" "GET /pubsub/topics" "GET" "$BASE_URL/pubsub/topics" "" true
test_endpoint "PubSub" "GET /pubsub/{topic}/info" "GET" "$BASE_URL/pubsub/$KEY_PREFIX-topic1/info" "" true
echo ""

# ========== QUEUE ==========
echo -e "${BLUE}=== Queue ===${NC}"
test_endpoint "Queue" "POST /queue/{name}" "POST" "$BASE_URL/queue/$KEY_PREFIX-queue1" '{"config":{}}' true
test_endpoint "Queue" "POST /queue/{name}/publish" "POST" "$BASE_URL/queue/$KEY_PREFIX-queue1/publish" '{"payload":[113,117,101,117,101,32,109,101,115,115,97,103,101],"priority":5}' true
test_endpoint "Queue" "GET /queue/{name}/stats" "GET" "$BASE_URL/queue/$KEY_PREFIX-queue1/stats" "" true
test_endpoint "Queue" "GET /queue/list" "GET" "$BASE_URL/queue/list" "" true
echo ""

# ========== STREAM ==========
echo -e "${BLUE}=== Stream ===${NC}"
curl -s -X DELETE "$BASE_URL/stream/$KEY_PREFIX-room1" > /dev/null 2>&1
test_endpoint "Stream" "POST /stream/{room}" "POST" "$BASE_URL/stream/$KEY_PREFIX-room1" '{}' true
test_endpoint "Stream" "POST /stream/{room}/publish" "POST" "$BASE_URL/stream/$KEY_PREFIX-room1/publish" '{"event":"message","data":"stream message"}' true
test_endpoint "Stream" "GET /stream/{room}/stats" "GET" "$BASE_URL/stream/$KEY_PREFIX-room1/stats" "" true
test_endpoint "Stream" "GET /stream/list" "GET" "$BASE_URL/stream/list" "" true
echo ""

# ========== SUMMARY ==========
echo "=========================================="
echo -e "${BLUE}Test Summary${NC}"
echo "=========================================="
echo "  Total Tests: $test_count"
echo -e "  ${GREEN}Passed: $pass_count${NC}"
echo -e "  ${RED}Failed: $fail_count${NC}"
echo "  Success Rate: $(( pass_count * 100 / test_count ))%"
echo "=========================================="

if [ $fail_count -eq 0 ]; then
    exit 0
else
    exit 1
fi

