#!/bin/bash
# Test Docker authentication setup

set -e

echo "=========================================="
echo "Testing Synap Docker Authentication Setup"
echo "=========================================="

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
IMAGE_NAME="synap:test-auth"
CONTAINER_NAME="synap-test-auth"
ROOT_USERNAME="admin"
ROOT_PASSWORD="SecurePassword123!"

echo -e "${YELLOW}Step 1: Building Docker image...${NC}"
docker build -t "$IMAGE_NAME" . || {
    echo -e "${RED}Failed to build Docker image${NC}"
    exit 1
}
echo -e "${GREEN}✓ Docker image built successfully${NC}"

echo -e "\n${YELLOW}Step 2: Starting container with authentication enabled...${NC}"
docker rm -f "$CONTAINER_NAME" 2>/dev/null || true

docker run -d \
    --name "$CONTAINER_NAME" \
    -p 15500:15500 \
    -e SYNAP_AUTH_ENABLED=true \
    -e SYNAP_AUTH_REQUIRE_AUTH=true \
    -e SYNAP_AUTH_ROOT_USERNAME="$ROOT_USERNAME" \
    -e SYNAP_AUTH_ROOT_PASSWORD="$ROOT_PASSWORD" \
    -e SYNAP_AUTH_ROOT_ENABLED=true \
    "$IMAGE_NAME" || {
    echo -e "${RED}Failed to start container${NC}"
    exit 1
}
echo -e "${GREEN}✓ Container started${NC}"

echo -e "\n${YELLOW}Step 3: Waiting for server to be ready...${NC}"
sleep 5

# Wait for health check
for i in {1..30}; do
    if curl -s http://localhost:15500/health > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Server is ready${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}Server failed to start${NC}"
        docker logs "$CONTAINER_NAME"
        exit 1
    fi
    sleep 1
done

echo -e "\n${YELLOW}Step 4: Testing unauthenticated access (should fail)...${NC}"
if curl -s -o /dev/null -w "%{http_code}" http://localhost:15500/health | grep -q "401"; then
    echo -e "${GREEN}✓ Unauthenticated access correctly rejected (401)${NC}"
else
    echo -e "${RED}✗ Unauthenticated access should return 401${NC}"
    docker logs "$CONTAINER_NAME" | tail -20
    exit 1
fi

echo -e "\n${YELLOW}Step 5: Testing Basic Auth (should succeed)...${NC}"
AUTH_HEADER=$(echo -n "$ROOT_USERNAME:$ROOT_PASSWORD" | base64)
if curl -s -H "Authorization: Basic $AUTH_HEADER" http://localhost:15500/health | grep -q "ok"; then
    echo -e "${GREEN}✓ Basic Auth successful${NC}"
else
    echo -e "${RED}✗ Basic Auth failed${NC}"
    docker logs "$CONTAINER_NAME" | tail -20
    exit 1
fi

echo -e "\n${YELLOW}Step 6: Testing API Key creation...${NC}"
# Create API key using Basic Auth
API_KEY_RESPONSE=$(curl -s -X POST \
    -H "Authorization: Basic $AUTH_HEADER" \
    -H "Content-Type: application/json" \
    -d '{"name": "test-key", "permissions": [{"resource": "*", "actions": ["all"]}], "expiresInSeconds": 3600}' \
    http://localhost:15500/auth/keys)

if echo "$API_KEY_RESPONSE" | grep -q "key"; then
    API_KEY=$(echo "$API_KEY_RESPONSE" | grep -o '"key":"[^"]*' | cut -d'"' -f4)
    echo -e "${GREEN}✓ API Key created: ${API_KEY:0:20}...${NC}"
else
    echo -e "${RED}✗ Failed to create API key${NC}"
    echo "Response: $API_KEY_RESPONSE"
    docker logs "$CONTAINER_NAME" | tail -20
    exit 1
fi

echo -e "\n${YELLOW}Step 7: Testing API Key authentication...${NC}"
if curl -s -H "Authorization: Bearer $API_KEY" http://localhost:15500/health | grep -q "ok"; then
    echo -e "${GREEN}✓ API Key authentication successful${NC}"
else
    echo -e "${RED}✗ API Key authentication failed${NC}"
    docker logs "$CONTAINER_NAME" | tail -20
    exit 1
fi

echo -e "\n${YELLOW}Step 8: Testing KV operations with authentication...${NC}"
if curl -s -X POST \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{"key": "test:docker:auth", "value": "success"}' \
    http://localhost:15500/kv/set | grep -q "success"; then
    echo -e "${GREEN}✓ KV SET with authentication successful${NC}"
else
    echo -e "${RED}✗ KV SET with authentication failed${NC}"
    docker logs "$CONTAINER_NAME" | tail -20
    exit 1
fi

if curl -s -H "Authorization: Bearer $API_KEY" \
    http://localhost:15500/kv/get/test:docker:auth | grep -q "success"; then
    echo -e "${GREEN}✓ KV GET with authentication successful${NC}"
else
    echo -e "${RED}✗ KV GET with authentication failed${NC}"
    docker logs "$CONTAINER_NAME" | tail -20
    exit 1
fi

echo -e "\n${YELLOW}Step 9: Cleaning up...${NC}"
docker rm -f "$CONTAINER_NAME" 2>/dev/null || true
echo -e "${GREEN}✓ Container removed${NC}"

echo -e "\n${GREEN}=========================================="
echo "All authentication tests passed! ✓"
echo "==========================================${NC}"

