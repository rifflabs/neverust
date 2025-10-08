# WantlistEntry Migration Guide

## Summary

Updated `WantlistEntry` to use `BlockAddress` instead of simple `Vec<u8>` for block identification, aligning with Archivist protocol compatibility requirements.

## Changes Made

### 1. New `BlockAddress` Struct

**Location**: `/opt/castle/workspace/neverust/neverust-core/src/messages.rs`

```rust
#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockAddress {
    /// Is this a Merkle tree leaf? (false = simple CID)
    #[prost(bool, tag = "1")]
    pub leaf: bool,

    /// Tree CID (only used when leaf=true)
    #[prost(bytes = "vec", tag = "2")]
    pub tree_cid: Vec<u8>,

    /// Index in tree (only used when leaf=true)
    #[prost(uint64, tag = "3")]
    pub index: u64,

    /// Simple CID (only used when leaf=false)
    #[prost(bytes = "vec", tag = "4")]
    pub cid: Vec<u8>,
}
```

**Helper Methods**:
- `BlockAddress::from_cid(cid: Vec<u8>)` - Create simple CID address
- `BlockAddress::from_tree_leaf(tree_cid: Vec<u8>, index: u64)` - Create Merkle tree leaf address
- `cid_bytes(&self) -> &[u8]` - Get CID bytes (works for both types)

### 2. Updated `WantlistEntry` Struct

**Before**:
```rust
pub struct WantlistEntry {
    pub block: Vec<u8>,              // Simple bytes
    pub priority: i32,
    pub cancel: bool,
    pub want_type: i32,
    pub send_dont_have: bool,
    pub start_byte: u64,             // ❌ REMOVED (incompatible extension)
    pub end_byte: u64,               // ❌ REMOVED (incompatible extension)
}
```

**After**:
```rust
pub struct WantlistEntry {
    pub address: Option<BlockAddress>,  // Complex structure
    pub priority: i32,
    pub cancel: bool,
    pub want_type: i32,
    pub send_dont_have: bool,
    // Range fields REMOVED - they broke Archivist compatibility
}
```

**Helper Methods**:
- `WantlistEntry::from_cid(cid: Vec<u8>, want_type: WantType)` - Create from simple CID
- `WantlistEntry::from_cid_struct(cid: &cid::Cid, want_type: WantType)` - Create from CID struct
- `WantlistEntry::from_tree_leaf(tree_cid: Vec<u8>, index: u64, want_type: WantType)` - Create for Merkle tree leaf
- `WantlistEntry::cancel_cid(cid: Vec<u8>)` - Create cancel entry
- `cid_bytes(&self) -> Option<&[u8]>` - Get CID bytes from address

### 3. Range Retrieval Fields Removed

**Why?**: The custom range fields (`start_byte`, `end_byte` in WantlistEntry) were a Neverust-specific extension that broke compatibility with Archivist-Node.

**Impact**:
- Range retrieval is still supported in `Block` payload (response side)
- Range requests in wantlist are removed (request side)
- Full blocks are now always requested

**Future**: If range requests are needed, they should be implemented using Archivist's protocol mechanisms, not custom extensions.

## Migration Steps

### For Code Using `WantlistEntry.block` Directly

**Before**:
```rust
let entry = WantlistEntry {
    block: cid.to_bytes(),
    priority: 1,
    cancel: false,
    want_type: WantType::WantBlock as i32,
    send_dont_have: true,
    start_byte: 0,
    end_byte: 0,
};

// Access CID
let cid_bytes = &entry.block;
```

**After**:
```rust
// Option 1: Use helper method (recommended)
let entry = WantlistEntry::from_cid(cid.to_bytes(), WantType::WantBlock);

// Option 2: Manual construction
let entry = WantlistEntry {
    address: Some(BlockAddress::from_cid(cid.to_bytes())),
    priority: 1,
    cancel: false,
    want_type: WantType::WantBlock as i32,
    send_dont_have: true,
};

// Access CID
let cid_bytes = entry.cid_bytes().unwrap();
```

### For Code Checking Range Fields

**Before**:
```rust
let is_range_request = entry.start_byte != 0 || entry.end_byte != 0;
if is_range_request {
    // Handle range request
} else {
    // Handle full block
}
```

**After**:
```rust
// Range requests no longer supported in WantlistEntry
// Always treat as full block request
// Range responses are still supported in Block payload
```

### For Code Creating Cancel Entries

**Before**:
```rust
let cancel_entry = WantlistEntry {
    block: cid.to_bytes(),
    priority: 0,
    cancel: true,
    want_type: WantType::WantBlock as i32,
    send_dont_have: false,
    start_byte: 0,
    end_byte: 0,
};
```

**After**:
```rust
let cancel_entry = WantlistEntry::cancel_cid(cid.to_bytes());
```

## Files Updated

### Modified Files
1. `/opt/castle/workspace/neverust/neverust-core/src/messages.rs`
   - Added `BlockAddress` struct with helper methods
   - Updated `WantlistEntry` struct to use `BlockAddress`
   - Removed `start_byte` and `end_byte` fields
   - Added helper methods for creating entries
   - Updated all tests

2. `/opt/castle/workspace/neverust/neverust-core/src/blockexc.rs`
   - Updated to use `entry.cid_bytes()` instead of `entry.block`
   - Removed range request handling in altruistic mode
   - Removed range request handling in marketplace mode
   - Updated outbound stream to use `WantlistEntry::from_cid()`

### Test Updates
- `test_encode_decode_wantlist()` - Uses new `BlockAddress::from_cid()`
- `test_roundtrip_complex_message()` - Uses new `BlockAddress::from_cid()`
- `test_block_address_simple_cid()` - NEW: Tests simple CID addressing
- `test_block_address_tree_leaf()` - NEW: Tests Merkle tree leaf addressing
- `test_wantlist_entry_from_cid()` - NEW: Tests helper method
- `test_wantlist_entry_from_tree_leaf()` - NEW: Tests Merkle tree leaf entries
- `test_wantlist_entry_cancel()` - NEW: Tests cancel entry creation
- `test_range_request_encoding()` - REMOVED (incompatible feature)
- `test_full_block_backward_compatible()` - REMOVED (no longer applicable)

## Compatibility Impact

### ✅ Archivist Protocol Alignment
- WantlistEntry now matches Archivist's `WantListEntry` structure
- Supports both simple CID and Merkle tree leaf addressing
- Eliminates custom field extensions that broke interoperability

### ❌ Breaking Changes
- Any code directly accessing `entry.block` must be updated
- Range request functionality in WantlistEntry removed
- Custom protobuf tags 6-7 no longer used in WantlistEntry

### ⚠️ Not Yet Implemented
- Merkle tree support (infrastructure exists, not yet used)
- `BlockDelivery` type (still using simple `Block`)
- `ArchivistProof` type (Merkle proofs)
- BlockPresence still uses simple `cid: Vec<u8>` (needs similar update)

## Testing

All tests pass except one pre-existing failure:
```
cargo test -p neverust-core
...
test result: FAILED. 69 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

The failing test (`cid_blake3::tests::test_blake3_cid`) is unrelated to WantlistEntry changes.

## Next Steps

1. **Update BlockPresence** - Apply similar BlockAddress migration
2. **Implement BlockDelivery** - Replace Block in Message.payload
3. **Add ArchivistProof** - Support Merkle tree proofs
4. **Integration Testing** - Test against real Archivist-Node instance
5. **Documentation** - Update API docs and examples

## References

- Archivist Compatibility Report: `/opt/castle/workspace/neverust/ARCHIVIST_COMPATIBILITY_REPORT.md`
- Original Issue: WantlistEntry uses incompatible field structure (Agent 9)
- Protobuf Definition: See Archivist-Node proto files
