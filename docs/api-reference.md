# Neverust API Reference

## REST API Endpoints

### Block Operations

#### GET /api/v1/blocks/:cid

Retrieve a block by CID. Supports HTTP Range headers for partial content retrieval.

**Parameters:**
- `cid` (path) - Content Identifier (CID) of the block

**Headers:**
- `Range` (optional) - HTTP Range header (e.g., `bytes=1024-2047`)

**Response (200 OK - Full Block):**
```json
{
  "cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
  "data": "base64-encoded-block-data",
  "size": 5000
}
```

**Response Headers:**
- `Accept-Ranges: bytes` - Indicates range support

**Response (206 Partial Content - Range Request):**
```json
{
  "cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
  "data": "base64-encoded-range-data",
  "size": 1024
}
```

**Response Headers:**
- `Content-Range: bytes 1024-2047/5000` - Indicates byte range returned
- `Accept-Ranges: bytes` - Indicates range support

**Example:**
```bash
# Full block
curl http://localhost:9080/api/v1/blocks/bafybeig...

# Range request (bytes 1024-2047)
curl -H "Range: bytes=1024-2047" http://localhost:9080/api/v1/blocks/bafybeig...

# Open-ended range (from 1024 to EOF)
curl -H "Range: bytes=1024-" http://localhost:9080/api/v1/blocks/bafybeig...
```

---

#### POST /api/v1/blocks

Store a block.

**Request Body:**
- Raw binary data (application/octet-stream)

**Response (200 OK):**
```json
{
  "cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
  "size": 1024
}
```

**Example:**
```bash
curl -X POST \
  --data-binary @file.bin \
  -H "Content-Type: application/octet-stream" \
  http://localhost:9080/api/v1/blocks
```

---

### Archivist-Compatible Endpoints

#### POST /api/archivist/v1/data

Upload data (Archivist-compatible endpoint). Returns CID as plain text.

**Request Body:**
- Raw binary data (application/octet-stream)

**Response (200 OK):**
```
bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi
```

**Example:**
```bash
CID=$(curl -X POST --data-binary @file.bin \
  http://localhost:9080/api/archivist/v1/data)
echo "Uploaded as: $CID"
```

---

#### GET /api/archivist/v1/data/:cid/network/stream

Download data (Archivist-compatible endpoint). Returns raw binary data.

**Parameters:**
- `cid` (path) - Content Identifier

**Response (200 OK):**
- Raw binary data (application/octet-stream)

**Example:**
```bash
curl http://localhost:9080/api/archivist/v1/data/bafybeig.../network/stream \
  -o downloaded.bin
```

---

### Health & Monitoring

#### GET /health

Health check endpoint.

**Response (200 OK):**
```json
{
  "status": "ok",
  "version": "0.1.0",
  "peer_id": "12D3KooWExample123...",
  "uptime_seconds": 3600
}
```

---

#### GET /metrics

Prometheus metrics endpoint.

**Response (200 OK):**
```
# HELP neverust_peer_count Number of connected peers
# TYPE neverust_peer_count gauge
neverust_peer_count 5

# HELP neverust_blocks_sent_total Total blocks sent
# TYPE neverust_blocks_sent_total counter
neverust_blocks_sent_total 42

# HELP neverust_blocks_received_total Total blocks received
# TYPE neverust_blocks_received_total counter
neverust_blocks_received_total 37

# HELP neverust_bytes_sent_total Total bytes sent
# TYPE neverust_bytes_sent_total counter
neverust_bytes_sent_total 1048576

# HELP neverust_bytes_received_total Total bytes received
# TYPE neverust_bytes_received_total counter
neverust_bytes_received_total 983040
```

---

## HTTP Range Request Specification

### Range Header Format

```
Range: bytes=<start>-<end>
```

- `start` - Start byte offset (inclusive)
- `end` - End byte offset (inclusive, optional)

### Range Types

| Format | Description | Example |
|--------|-------------|---------|
| `bytes=0-1023` | First 1024 bytes | Bytes 0-1023 |
| `bytes=1024-2047` | Specific range | Bytes 1024-2047 |
| `bytes=1024-` | From offset to EOF | Bytes 1024 to end |
| `bytes=-1024` | Last 1024 bytes | Last 1024 bytes |

### Content-Range Response Format

```
Content-Range: bytes <start>-<end>/<total>
```

- `start` - Start byte offset (inclusive)
- `end` - End byte offset (inclusive)
- `total` - Total size of the block

**Example:**
```
Content-Range: bytes 1024-2047/5000
```
Indicates bytes 1024-2047 of a 5000-byte block (1024 bytes returned).

---

## BlockExc Protocol (P2P)

### WantlistEntry (Request)

```protobuf
message Entry {
  bytes block = 1;           // CID bytes
  int32 priority = 2;        // Priority (1-10)
  bool cancel = 3;           // Cancel request
  WantType wantType = 4;     // WantBlock or WantHave
  bool sendDontHave = 5;     // Request negative acknowledgment
  uint64 startByte = 6;      // Range start (0 = full block)
  uint64 endByte = 7;        // Range end (0 = full block)
}
```

**Range Request Example:**
```rust
WantlistEntry {
    block: cid.to_bytes(),
    priority: 100,
    cancel: false,
    want_type: WantType::WantBlock as i32,
    send_dont_have: true,
    start_byte: 1024,  // Request bytes 1024-2048
    end_byte: 2048,
}
```

---

### Block (Response)

```protobuf
message Block {
  bytes prefix = 1;       // CID prefix
  bytes data = 2;         // Block data (or range data)
  uint64 rangeStart = 3;  // Range start (0 = full block)
  uint64 rangeEnd = 4;    // Range end (0 = full block)
  uint64 totalSize = 5;   // Total block size (0 = full block)
}
```

**Range Response Example:**
```rust
Block {
    prefix: cid.to_bytes()[0..4].to_vec(),
    data: vec![...], // 1024 bytes
    range_start: 1024,
    range_end: 2048,
    total_size: 5000,
}
```

---

## Peer Capability Detection

### Identify Protocol

Neverust uses libp2p's identify protocol to detect peer capabilities:

**Neverust Peer (supports ranges):**
```
agent_version: "/neverust/0.1.0"
protocol_version: "/neverust/0.1.0"
protocols: ["/archivist/blockexc/1.0.0", "/ipfs/id/1.0.0"]
```

**Archivist-Node Peer (full blocks only):**
```
agent_version: "archivist/1.2.3"
protocol_version: "ipfs/0.1.0"
protocols: ["/archivist/blockexc/1.0.0", "/ipfs/id/1.0.0"]
```

### Capability Registry

Stored in `PeerRegistry` (`Arc<RwLock<HashMap<PeerId, PeerCapability>>>`):

```rust
struct PeerCapability {
    supports_ranges: bool,        // true if "/neverust/" in agent_version
    agent_version: String,        // e.g., "/neverust/0.1.0"
    protocol_version: String,     // e.g., "/neverust/0.1.0"
}
```

---

## Error Responses

### 400 Bad Request

Invalid request (e.g., malformed CID, invalid range).

```json
{
  "error": "Invalid CID: failed to parse"
}
```

---

### 404 Not Found

Block not found in local store.

```json
{
  "error": "Block not found: bafybeig..."
}
```

---

### 416 Range Not Satisfiable

Requested range is invalid (e.g., start ≥ block size).

```json
{
  "error": "Range not satisfiable"
}
```

---

### 500 Internal Server Error

Server-side error.

```json
{
  "error": "Internal server error: ..."
}
```

---

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LISTEN_PORT` | TCP listen port | 9000 |
| `API_PORT` | HTTP API port | 9080 |
| `DISC_PORT` | UDP discovery port | 8090 |
| `MODE` | Operating mode (altruistic/marketplace) | altruistic |
| `PRICE_PER_BYTE` | Price per byte (marketplace mode) | 1 |
| `DATA_DIR` | Data directory for RocksDB | ./data |

---

## Client SDKs

### JavaScript/TypeScript

```typescript
class NeverustClient {
  constructor(private baseURL: string) {}

  async getBlock(cid: string): Promise<{ data: string; size: number }> {
    const response = await fetch(`${this.baseURL}/api/v1/blocks/${cid}`);
    return response.json();
  }

  async getRange(
    cid: string,
    start: number,
    end: number
  ): Promise<{ data: string; size: number }> {
    const response = await fetch(`${this.baseURL}/api/v1/blocks/${cid}`, {
      headers: { Range: `bytes=${start}-${end - 1}` },
    });
    return response.json();
  }

  async putBlock(data: Uint8Array): Promise<{ cid: string; size: number }> {
    const response = await fetch(`${this.baseURL}/api/v1/blocks`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream' },
      body: data,
    });
    return response.json();
  }
}

// Usage
const client = new NeverustClient('http://localhost:9080');
const range = await client.getRange(cid, 1024, 2048);
```

---

### Rust

```rust
use reqwest::header::{CONTENT_TYPE, RANGE};

async fn get_block(cid: &str) -> Result<Vec<u8>, reqwest::Error> {
    let response = reqwest::get(format!("http://localhost:9080/api/v1/blocks/{}", cid))
        .await?
        .json::<serde_json::Value>()
        .await?;

    let data_b64 = response["data"].as_str().unwrap();
    Ok(base64::decode(data_b64).unwrap())
}

async fn get_range(cid: &str, start: usize, end: usize) -> Result<Vec<u8>, reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://localhost:9080/api/v1/blocks/{}", cid))
        .header(RANGE, format!("bytes={}-{}", start, end - 1))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let data_b64 = response["data"].as_str().unwrap();
    Ok(base64::decode(data_b64).unwrap())
}
```

---

## Rate Limiting

Currently no rate limiting. Future versions may implement:
- Per-IP rate limits
- Per-peer rate limits
- Bandwidth throttling

---

## Security

### HTTPS/TLS

Not currently supported. For production deployments, use a reverse proxy (nginx, Caddy) for TLS termination:

```nginx
server {
    listen 443 ssl;
    server_name api.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:9080;
        proxy_set_header Host $host;
        proxy_set_header Range $http_range;
    }
}
```

### CORS

Not currently supported. Add via reverse proxy if needed:

```nginx
add_header Access-Control-Allow-Origin *;
add_header Access-Control-Allow-Methods "GET, POST, OPTIONS";
add_header Access-Control-Allow-Headers "Range, Content-Type";
```

---

## Performance Tuning

### RocksDB Configuration

Located in `neverust-core/src/storage.rs`:

```rust
opts.optimize_for_point_lookup(256);  // 256MB block cache
opts.set_enable_pipelined_write(true);
opts.set_write_buffer_size(64 * 1024 * 1024);  // 64MB
opts.set_target_file_size_base(128 * 1024 * 1024);  // 128MB
```

### Connection Pool

libp2p connection limits (located in `neverust-core/src/p2p.rs`):

```rust
.with_idle_connection_timeout(Duration::from_secs(300))  // 5 minutes
```

---

## See Also

- [Range Retrieval Documentation](./range-retrieval.md)
- [README.md](../README.md)
- [Archivist BlockExc Spec](https://github.com/ipfs/specs/blob/main/BITSWAP.md)
