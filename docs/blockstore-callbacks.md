# BlockStore Callback System

## Overview

The BlockStore now supports callback hooks that are automatically invoked when new blocks are stored. This enables automatic block announcement to the DHT without manual intervention.

## Implementation

### BlockStore Changes

**File**: `/opt/castle/workspace/neverust/neverust-core/src/storage.rs`

#### Added Field
```rust
pub struct BlockStore {
    db: Arc<DB>,
    /// Callback invoked when a new block is stored (for announcing to network)
    on_block_stored: Option<Arc<dyn Fn(Cid) + Send + Sync>>,
}
```

#### New Method
```rust
/// Register a callback to be invoked when a new block is stored
///
/// This callback is called asynchronously after successful storage,
/// and can be used to announce new blocks to the network.
pub fn set_on_block_stored(&mut self, callback: Arc<dyn Fn(Cid) + Send + Sync>) {
    self.on_block_stored = Some(callback);
}
```

#### Modified `put()` Method
The `put()` method now:
1. Tracks whether a block is new (not a duplicate)
2. Only invokes the callback for newly stored blocks
3. Spawns callback asynchronously to avoid blocking storage operations

```rust
if was_new_block {
    info!("Stored block {}, size: {} bytes", cid_str, block.data.len());

    // Invoke callback asynchronously if registered
    if let Some(callback) = &self.on_block_stored {
        let callback = Arc::clone(callback);
        let cid = block.cid;
        tokio::spawn(async move {
            callback(cid);
        });
    }
}
```

## Usage with Advertiser

The Advertiser module (added in `/opt/castle/workspace/neverust/neverust-core/src/advertiser.rs`) provides automatic DHT announcements with:
- Queue-based processing to avoid overwhelming DHT
- Concurrent limiting (default: 10 concurrent announcements)
- Periodic re-advertisement (default: every 30 minutes)

### Integration Example

```rust
use neverust_core::{BlockStore, Advertiser, Discovery};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create discovery service
    let keypair = libp2p::identity::Keypair::generate_secp256k1();
    let listen_addr = "127.0.0.1:9000".parse()?;
    let discovery = Arc::new(
        Discovery::new(&keypair, listen_addr, vec![], vec![]).await?
    );

    // Create advertiser with default settings (10 concurrent, 30 min re-advertisement)
    let advertiser = Arc::new(Advertiser::with_defaults(Arc::clone(&discovery)));
    advertiser.start().await?;

    // Create block store
    let mut store = BlockStore::new_with_path("./data/blocks")?;

    // Register callback to automatically announce blocks
    let advertiser_clone = Arc::clone(&advertiser);
    store.set_on_block_stored(Arc::new(move |cid| {
        let advertiser = Arc::clone(&advertiser_clone);
        tokio::spawn(async move {
            if let Err(e) = advertiser.advertise_block(&cid).await {
                warn!("Failed to advertise block {}: {}", cid, e);
            }
        });
    }));

    // Now every new block stored will be automatically announced!
    let data = b"hello world".to_vec();
    let block = Block::new(data)?;
    store.put(block).await?; // This triggers the callback

    Ok(())
}
```

## Callback Characteristics

### Non-Blocking
The callback is spawned in a separate tokio task, ensuring storage operations complete quickly:

```rust
// Storage completes in < 100ms even with slow callback
tokio::spawn(async move {
    callback(cid);  // Runs asynchronously
});
```

### Duplicate Detection
Callbacks are only invoked for newly stored blocks, not duplicates:

```rust
store.put(block.clone()).await?; // Callback invoked
store.put(block.clone()).await?; // Callback NOT invoked (duplicate)
```

### Error Resilience
Callback failures don't affect storage operations. The callback should handle errors internally.

## Testing

Three comprehensive tests verify callback behavior:

1. **`test_on_block_stored_callback`**: Verifies callbacks are invoked for stored blocks
2. **`test_callback_not_invoked_for_duplicate_blocks`**: Ensures duplicates don't trigger callbacks
3. **`test_callback_does_not_block_storage`**: Confirms async execution doesn't block storage

Run tests with:
```bash
cargo test -p neverust-core storage::tests::test.*callback
```

## Architecture Alignment

This implementation follows the Archivist pattern from `advertiser.nim:137-138`:

```nim
# Archivist pattern
proc newAdvertiser*(...): Advertiser =
  result.blockProcessor.onBlockStored = proc(block: Block) =
    result.onBlockStored(block)

proc onBlockStored*(advertiser: Advertiser, block: Block) =
  if advertiser.enabled:
    for validator in advertiser.validators:
      advertiser.advertise(validator, block)
```

Our Rust implementation:
- **BlockStore**: Equivalent to `blockProcessor`
- **Callback**: Equivalent to `onBlockStored` lambda
- **Advertiser**: Equivalent to `advertiser.advertise()`

## Benefits

1. **Automatic Announcements**: No manual intervention needed
2. **Decoupled Design**: Storage and announcement are independent
3. **Performance**: Async callbacks don't block storage
4. **Idempotent**: Duplicate blocks don't cause redundant announcements
5. **Flexible**: Any callback logic can be registered (logging, metrics, etc.)

## Future Enhancements

Potential improvements:
- Support multiple callbacks (callback chain)
- Callback priority system
- Callback error reporting channel
- Conditional callbacks (e.g., only for blocks > 1MB)

## References

- **Archivist Reference**: `codex-storage/nim-codex` - `advertiser.nim:137-138`
- **Implementation**: `/opt/castle/workspace/neverust/neverust-core/src/storage.rs`
- **Usage Example**: `/opt/castle/workspace/neverust/neverust-core/src/advertiser.rs`
