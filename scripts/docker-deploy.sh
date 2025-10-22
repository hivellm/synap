#!/bin/bash
# ============================================================================
# Synap Docker Deployment Script
# ============================================================================
#
# This script manages the Synap replication cluster using Docker Compose
#
# Usage:
#   ./scripts/docker-deploy.sh [command]
#
# Commands:
#   start     - Start the cluster (1 master + 3 replicas)
#   stop      - Stop the cluster
#   restart   - Restart the cluster
#   status    - Show cluster status
#   logs      - Show logs (all nodes)
#   health    - Check health of all nodes
#   clean     - Stop and remove all data (DANGER!)
#
# Examples:
#   ./scripts/docker-deploy.sh start
#   ./scripts/docker-deploy.sh logs
#   ./scripts/docker-deploy.sh health
#
# ============================================================================

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
COMPOSE_FILE="docker-compose.yml"
MASTER_PORT=15500
REPLICA1_PORT=15510
REPLICA2_PORT=15520
REPLICA3_PORT=15530

# Helper functions
print_header() {
    echo -e "${BLUE}============================================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}============================================================================${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_info() {
    echo -e "${CYAN}ℹ $1${NC}"
}

check_health() {
    local name=$1
    local port=$2
    
    if curl -s -f "http://localhost:${port}/health" > /dev/null 2>&1; then
        print_success "${name} is healthy (port ${port})"
        return 0
    else
        print_error "${name} is unhealthy (port ${port})"
        return 1
    fi
}

# Commands
cmd_start() {
    print_header "Starting Synap Replication Cluster"
    
    if [ ! -f "$COMPOSE_FILE" ]; then
        print_error "docker-compose.yml not found"
        exit 1
    fi
    
    print_info "Building images..."
    docker-compose build
    
    print_info "Starting services..."
    docker-compose up -d
    
    echo ""
    print_info "Waiting for services to be healthy..."
    sleep 5
    
    echo ""
    print_success "Cluster started!"
    echo ""
    cmd_status
}

cmd_stop() {
    print_header "Stopping Synap Replication Cluster"
    
    docker-compose down
    
    echo ""
    print_success "Cluster stopped"
}

cmd_restart() {
    print_header "Restarting Synap Replication Cluster"
    
    cmd_stop
    sleep 2
    cmd_start
}

cmd_status() {
    print_header "Synap Cluster Status"
    
    docker-compose ps
    
    echo ""
    print_info "Service endpoints:"
    echo "  Master (write):  http://localhost:${MASTER_PORT}"
    echo "  Replica 1 (read): http://localhost:${REPLICA1_PORT}"
    echo "  Replica 2 (read): http://localhost:${REPLICA2_PORT}"
    echo "  Replica 3 (read): http://localhost:${REPLICA3_PORT}"
}

cmd_logs() {
    print_header "Synap Cluster Logs"
    
    docker-compose logs -f
}

cmd_health() {
    print_header "Synap Cluster Health Check"
    
    echo "Checking node health..."
    echo ""
    
    check_health "Master" $MASTER_PORT
    check_health "Replica 1" $REPLICA1_PORT
    check_health "Replica 2" $REPLICA2_PORT
    check_health "Replica 3" $REPLICA3_PORT
    
    echo ""
    print_info "Checking replication status..."
    echo ""
    
    # Check master replication status
    echo "Master replication status:"
    curl -s "http://localhost:${MASTER_PORT}/health/replication" | jq '.' || echo "Not available"
    echo ""
    
    # Check replica 1 status
    echo "Replica 1 replication status:"
    curl -s "http://localhost:${REPLICA1_PORT}/health/replication" | jq '.' || echo "Not available"
}

cmd_clean() {
    print_header "Clean Synap Cluster (DANGER!)"
    
    print_warning "This will stop all containers and DELETE ALL DATA!"
    read -p "Are you sure? (yes/no): " -r
    echo ""
    
    if [[ $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
        docker-compose down -v
        print_success "Cluster stopped and all data removed"
    else
        print_info "Operation cancelled"
    fi
}

# Main
COMMAND="${1:-help}"

case "$COMMAND" in
    start)
        cmd_start
        ;;
    stop)
        cmd_stop
        ;;
    restart)
        cmd_restart
        ;;
    status)
        cmd_status
        ;;
    logs)
        cmd_logs
        ;;
    health)
        cmd_health
        ;;
    clean)
        cmd_clean
        ;;
    help|*)
        print_header "Synap Docker Deployment"
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  start     - Start the cluster (1 master + 3 replicas)"
        echo "  stop      - Stop the cluster"
        echo "  restart   - Restart the cluster"
        echo "  status    - Show cluster status"
        echo "  logs      - Show logs (all nodes)"
        echo "  health    - Check health of all nodes"
        echo "  clean     - Stop and remove all data (DANGER!)"
        echo ""
        echo "Examples:"
        echo "  $0 start"
        echo "  $0 logs"
        echo "  $0 health"
        ;;
esac

