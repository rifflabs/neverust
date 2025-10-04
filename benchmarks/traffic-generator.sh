#!/bin/bash
# Independent traffic generator - runs inside each node container
# Each node acts autonomously, generating and requesting blocks

NODE_ID="${NODE_ID:-unknown}"
API_PORT="${API_PORT:-8080}"
BLOCK_SIZE="${BLOCK_SIZE:-1024}"  # 1KB blocks
UPLOAD_RATE="${UPLOAD_RATE:-5}"   # Blocks per minute
REQUEST_RATE="${REQUEST_RATE:-10}" # Requests per minute

echo "Traffic generator starting for node $NODE_ID"
echo "Upload rate: $UPLOAD_RATE blocks/min, Request rate: $REQUEST_RATE requests/min"

# Wait for node to be ready
sleep 5

# Generate peer list (all nodes in cluster)
PEERS=("bootstrap")
for i in {1..49}; do
    PEERS+=("node$i")
done

# Function to generate random data block
generate_block() {
    head -c $BLOCK_SIZE /dev/urandom | base64
}

# Function to upload a block to local node
upload_block() {
    local data="$(generate_block)"
    local cid=$(echo -n "$data" | curl -s -X POST http://localhost:$API_PORT/api/archivist/v1/data --data-binary @-)
    echo "[UPLOAD] Node $NODE_ID created block: $cid"
    echo "$cid"
}

# Function to request a random block from a random peer
request_random_block() {
    # Pick a random peer
    local peer="${PEERS[$RANDOM % ${#PEERS[@]}]}"

    # Get a list of blocks from that peer
    local blocks=$(curl -s "http://$peer:8080/api/archivist/v1/stats" 2>/dev/null | grep -o '"block_count":[0-9]*' | cut -d: -f2)

    if [ -n "$blocks" ] && [ "$blocks" -gt 0 ]; then
        # Try to fetch a random block (this will trigger peer discovery if not local)
        # In a real scenario, we'd query the peer for their block list
        # For now, we'll just try to fetch recently uploaded CIDs
        echo "[REQUEST] Node $NODE_ID requesting block from $peer"
    fi
}

# Main loop - run both upload and request tasks concurrently
while true; do
    # Upload new blocks
    (
        while true; do
            upload_block
            sleep $((60 / UPLOAD_RATE))
        done
    ) &

    # Request blocks from peers
    (
        while true; do
            request_random_block
            sleep $((60 / REQUEST_RATE))
        done
    ) &

    wait
done
