# Range Retrieval in Neverust

Neverust extends the Archivist BlockExc protocol with range retrieval capabilities, enabling efficient partial block fetching for streaming, progressive loading, and bandwidth optimization.

## Overview

Range retrieval allows clients to request specific byte ranges of blocks instead of downloading entire blocks. This is critical for:

- **Streaming media** - Fetch video/audio chunks on-demand
- **Progressive loading** - Load large files incrementally
- **Bandwidth optimization** - Download only needed portions
- **Random access** - Seek to specific offsets without full download

## Architecture

### Protocol Extensions

Neverust extends Archivist's protobuf messages with range fields:

**WantlistEntry (Request):**
```protobuf
message Entry {
  bytes block = 1;
  int32 priority = 2;
  bool cancel = 3;
  WantType wantType = 4;
  bool sendDontHave = 5;
  uint64 startByte = 6;  // Range start (inclusive), 0 = from beginning
  uint64 endByte = 7;    // Range end (exclusive), 0 = full block
}
```

**Block (Response):**
```protobuf
message Block {
  bytes prefix = 1;
  bytes data = 2;
  uint64 rangeStart = 3;  // Starting byte offset (inclusive)
  uint64 rangeEnd = 4;    // Ending byte offset (exclusive)
  uint64 totalSize = 5;   // Total size of complete block
}
```

### Backward Compatibility

Range fields default to `0`, which indicates full block request/response:
- **Neverust ↔ Neverust**: Full range retrieval support
- **Neverust ↔ Archivist-Node**: Falls back to full blocks (zero range fields ignored)
- **Archivist-Node ↔ Archivist-Node**: No change (zero range fields)

### Peer Capability Detection

Neverust uses libp2p's identify protocol to detect peer capabilities:

```rust
// Identify event handler stores peer capabilities
Event::Received { peer_id, info } => {
    let supports_ranges = info.agent_version.contains("/neverust/");

    // Store in registry
    peer_registry.insert(peer_id, PeerCapability {
        supports_ranges,
        agent_version: info.agent_version,
        protocol_version: info.protocol_version,
    });
}
```

**Detection Logic:**
- Agent version contains "/neverust/" → Supports ranges
- All others → Requires full blocks (Archivist-Node)

## HTTP API Range Requests

Neverust implements standard HTTP Range headers for web clients.

### Request Format

```http
GET /api/v1/blocks/{cid} HTTP/1.1
Range: bytes=1024-2047
```

### Response Formats

**Partial Content (206):**
```http
HTTP/1.1 206 Partial Content
Content-Range: bytes 1024-2047/5000
Accept-Ranges: bytes

{
  "cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
  "data": "base64-encoded-range-data",
  "size": 1024
}
```

**Full Block (200):**
```http
HTTP/1.1 200 OK
Accept-Ranges: bytes

{
  "cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
  "data": "base64-encoded-full-data",
  "size": 5000
}
```

### Range Header Formats

| Range Header | Description | Response |
|--------------|-------------|----------|
| `bytes=0-1023` | First 1KB | `bytes 0-1023/5000` |
| `bytes=1024-2047` | Second 1KB | `bytes 1024-2047/5000` |
| `bytes=1024-` | From 1KB to end | `bytes 1024-4999/5000` |
| (no header) | Full block | `bytes */5000` (implied) |

## Usage Examples

### JavaScript/TypeScript (Web Client)

```javascript
// localhost:5175 or riff.cc
const cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";

// Fetch specific range (bytes 1024-2047)
async function fetchRange(cid, start, end) {
  const response = await fetch(`http://localhost:9080/api/v1/blocks/${cid}`, {
    headers: { 'Range': `bytes=${start}-${end - 1}` } // end is inclusive in HTTP
  });

  if (response.status === 206) {
    console.log('✓ Partial content received');
    console.log('Content-Range:', response.headers.get('Content-Range'));
  }

  const data = await response.json();
  const bytes = atob(data.data); // Decode base64
  return bytes;
}

// Progressive streaming
async function streamBlock(cid, chunkSize = 1024) {
  const chunks = [];
  let offset = 0;

  while (true) {
    const end = offset + chunkSize;
    const chunk = await fetchRange(cid, offset, end);

    if (chunk.length === 0) break;
    chunks.push(chunk);
    offset = end;

    console.log(`Downloaded ${offset} bytes...`);
  }

  return chunks.join('');
}

// Usage
await streamBlock(cid, 1024); // Stream in 1KB chunks
```

### curl (CLI)

```bash
# Fetch full block
curl http://localhost:9080/api/v1/blocks/bafybeig...

# Fetch bytes 1024-2047 (1KB)
curl -H "Range: bytes=1024-2047" \
     http://localhost:9080/api/v1/blocks/bafybeig...

# Fetch from offset 1024 to end
curl -H "Range: bytes=1024-" \
     http://localhost:9080/api/v1/blocks/bafybeig...

# Check if ranges supported
curl -I http://localhost:9080/api/v1/blocks/bafybeig... | grep Accept-Ranges
# Output: Accept-Ranges: bytes
```

### Rust (P2P Client)

```rust
use neverust_core::messages::{WantlistEntry, WantType};

// Request range from Neverust peer
let entry = WantlistEntry {
    block: cid.to_bytes(),
    priority: 100,
    cancel: false,
    want_type: WantType::WantBlock as i32,
    send_dont_have: true,
    start_byte: 1024,  // Request bytes 1024-2047
    end_byte: 2048,
};

// Send via BlockExc protocol
// Response will contain:
// - data: Vec<u8> (1024 bytes)
// - range_start: 1024
// - range_end: 2048
// - total_size: 5000
```

## Performance Considerations

### Bandwidth Savings

Example: 5MB video, client wants to preview first 10 seconds (500KB):

- **Without ranges**: Download 5MB → 5000KB transferred
- **With ranges**: `bytes=0-512000` → 500KB transferred
- **Savings**: 90% bandwidth reduction

### Latency Optimization

Range retrieval enables:
1. **Immediate playback** - Fetch first chunk, start playing
2. **Adaptive streaming** - Adjust chunk size based on network
3. **Seek optimization** - Jump to timestamp without re-download

### P2P Routing

Neverust optimizes P2P routing for range requests:

```rust
// Prefer Neverust peers for range requests
if let Some(capability) = peer_registry.get(&peer_id) {
    if capability.supports_ranges {
        // Send range request directly
        send_range_request(peer_id, start, end);
    } else {
        // Fallback: fetch full block, extract range locally
        let full_block = fetch_full_block(peer_id);
        return full_block.data[start..end];
    }
}
```

## Implementation Details

### Server-Side Range Extraction

Located in `neverust-core/src/blockexc.rs:171`:

```rust
// Check if this is a range request
let is_range_request = entry.start_byte != 0 || entry.end_byte != 0;

if is_range_request {
    let start = entry.start_byte as usize;
    let end = std::cmp::min(entry.end_byte as usize, total_size);

    let range_data = block.data[start..end].to_vec();

    response_blocks.push(MsgBlock {
        prefix: cid.to_bytes()[0..4].to_vec(),
        data: range_data,
        range_start: start as u64,
        range_end: end as u64,
        total_size,
    });
}
```

### HTTP Range Parser

Located in `neverust-core/src/api.rs:231`:

```rust
fn parse_range_header(range_str: &str, total_size: usize) -> Option<(usize, usize)> {
    let range_str = range_str.trim().strip_prefix("bytes=")?;
    let parts: Vec<&str> = range_str.split('-').collect();

    let start: usize = parts[0].parse().ok()?;
    let end: usize = if parts[1].is_empty() {
        total_size
    } else {
        parts[1].parse::<usize>().ok()? + 1 // HTTP end is inclusive
    };

    Some((start, std::cmp::min(end, total_size)))
}
```

### Peer Capability Registry

Located in `neverust-core/src/p2p.rs:16`:

```rust
pub struct PeerCapability {
    pub supports_ranges: bool,
    pub agent_version: String,
    pub protocol_version: String,
}

pub type PeerRegistry = Arc<RwLock<HashMap<PeerId, PeerCapability>>>;
```

## Testing

### Unit Tests

Located in `neverust-core/src/messages.rs:309`:

```rust
#[test]
fn test_range_request_encoding() {
    let entry = WantlistEntry {
        start_byte: 1024,
        end_byte: 2048,
        // ...
    };

    let encoded = encode_message(&msg).unwrap();
    let decoded = decode_message(&encoded).unwrap();

    assert_eq!(decoded.wantlist.entries[0].start_byte, 1024);
    assert_eq!(decoded.wantlist.entries[0].end_byte, 2048);
}
```

### Integration Tests

```bash
# Start Neverust node
cargo run --release -- start --log-level info

# In another terminal:
# Upload test block
echo "Hello, Range Retrieval!" > test.txt
CID=$(curl -X POST --data-binary @test.txt http://localhost:9080/api/archivist/v1/data)

# Test full block
curl http://localhost:9080/api/v1/blocks/$CID

# Test range request
curl -H "Range: bytes=0-4" http://localhost:9080/api/v1/blocks/$CID
# Should return: {"cid":"...","data":"SGVsbG8=","size":5}
# (base64 decode: "Hello")
```

## Troubleshooting

### Range Request Returns 200 Instead of 206

**Cause**: Range header malformed or server doesn't support ranges

**Solution**:
```bash
# Check Accept-Ranges header
curl -I http://localhost:9080/api/v1/blocks/$CID
# Should see: Accept-Ranges: bytes

# Verify Range header format
curl -v -H "Range: bytes=0-1023" http://localhost:9080/api/v1/blocks/$CID
```

### P2P Range Request Returns Full Block

**Cause**: Peer is Archivist-Node (doesn't support ranges)

**Solution**: Check logs for peer capability detection:
```
INFO Identified peer ...: agent_version=archivist/...
INFO Peer ... is Archivist-Node (requires full blocks)
```

Neverust automatically falls back to full blocks for Archivist-Node peers.

### Invalid Range Returns Error

**Cause**: Range exceeds block size or start ≥ end

**Solution**: Validate range before request:
```javascript
function validateRange(start, end, totalSize) {
  if (start >= totalSize) throw new Error('Start exceeds block size');
  if (start >= end) throw new Error('Start must be less than end');
  return { start, end: Math.min(end, totalSize) };
}
```

## Future Enhancements

- **Multi-range requests**: `bytes=0-1023,2048-3071`
- **Client-side caching**: Store ranges in local cache
- **Prefetching**: Predict next ranges for streaming
- **Compression**: Compress ranges before transfer
- **P2P routing optimization**: Prefer nearby Neverust peers

## References

- **Archivist BlockExc Protocol**: https://github.com/ipfs/specs/blob/main/BITSWAP.md
- **HTTP Range Requests (RFC 7233)**: https://tools.ietf.org/html/rfc7233
- **Neverust Protobuf**: `neverust-core/proto/message.proto`
- **Implementation**: `neverust-core/src/{blockexc.rs,api.rs,p2p.rs}`

## Support

For issues or questions:
- GitHub Issues: https://github.com/durability-labs/neverust/issues
- Documentation: https://docs.archivist.storage
