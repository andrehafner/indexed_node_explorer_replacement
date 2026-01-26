#!/bin/bash
# Quick start script for ergo-index
#
# Usage:
#   ./start.sh                    # Connect to local node at localhost:9053
#   ./start.sh external           # Same as above
#   ./start.sh embedded           # Start with embedded node
#   ./start.sh multi              # Use multiple public nodes for faster sync
#   ./start.sh stop               # Stop all containers

set -e

MODE=${1:-external}

case $MODE in
    external)
        echo "Starting ergo-index (external node mode)..."
        echo "Expecting Ergo node at: ${ERGO_NODES:-http://localhost:9053}"
        docker-compose up -d
        ;;

    embedded)
        echo "Starting ergo-index with embedded Ergo node..."
        echo "Note: The node will need time to sync before indexing can progress."
        docker-compose -f docker-compose.yml -f docker-compose.embedded.yml up -d
        ;;

    multi)
        echo "Starting ergo-index with multiple public nodes..."
        docker-compose -f docker-compose.yml -f docker-compose.multi-node.yml up -d
        ;;

    stop)
        echo "Stopping ergo-index..."
        docker-compose -f docker-compose.yml -f docker-compose.embedded.yml down 2>/dev/null || \
        docker-compose down
        ;;

    logs)
        docker-compose logs -f
        ;;

    status)
        curl -s http://localhost:8080/status | jq .
        ;;

    *)
        echo "Usage: $0 {external|embedded|multi|stop|logs|status}"
        echo ""
        echo "  external  - Connect to external node (default)"
        echo "  embedded  - Start with embedded Ergo node"
        echo "  multi     - Use multiple public nodes"
        echo "  stop      - Stop all containers"
        echo "  logs      - Follow container logs"
        echo "  status    - Show sync status"
        exit 1
        ;;
esac

echo ""
echo "Web UI:     http://localhost:8080"
echo "API Docs:   http://localhost:8080/docs"
echo "Status:     http://localhost:8080/status"
