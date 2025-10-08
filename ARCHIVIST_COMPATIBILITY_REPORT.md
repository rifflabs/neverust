# Archivist Compatibility Analysis Report

**Date**: 2025-10-08
**Analysis**: Deep dive into archivist-node codebase
**Issue**: UnexpectedEof when connecting to Archivist testnet

---

## 🎯 ROOT CAUSE IDENTIFIED

The `UnexpectedEof` error is caused by **incompatible protobuf message formats** between Neverust and Archivist-Node.

### Critical Incompatibilities

#### 1. **Payload Field Type Mismatch** ⚠️ CRITICAL

**Archivist expects:**
```protobuf
message Message {
  repeated BlockDelivery payload = 3;  // NOT Block!
}

message BlockDelivery {
  bytes cid = 1;        // Block CID
  bytes data = 2;       // Block data
  BlockAddress address = 3;  // Can be simple CID or tree CID+index
  ArchivistProof proof = 4;  // Merkle proof (optional, only for leaves)
}
```

**Neverust currently sends:**
```protobuf
message Message {
  repeated Block payload = 3;  // ← WRONG TYPE!
}

message Block {
  bytes prefix = 1;
  bytes data = 2;
  // Missing: address field, proof field
}
```

When Neverust sends a `Block` where Archivist expects `BlockDelivery`, the protobuf decoder fails or receives malformed data, causing the connection to close.

#### 2. **WantlistEntry Field Structure** ⚠️ CRITICAL

**Archivist uses BlockAddress (complex structure):**
```protobuf
message WantListEntry {
  BlockAddress address = 1;  // NOT simple bytes!
  int32 priority = 2;
  bool cancel = 3;
  WantType wantType = 4;
  bool sendDontHave = 5;
}

message BlockAddress {
  bool leaf = 1;              // Is this a Merkle tree leaf?
  bytes treeCid = 2;          // Tree CID (if leaf=true)
  uint64 index = 3;           // Index in tree (if leaf=true)
  bytes cid = 4;              // Simple CID (if leaf=false)
}
```

**Neverust uses simple bytes:**
```protobuf
message WantlistEntry {
  bytes block = 1;  // ← Too simple! Can't represent tree leaves
  int32 priority = 2;
  bool cancel = 3;
  WantType want_type = 4;
  bool send_dont_have = 5;
  // Extra fields 6-7 for range retrieval (Neverust extension)
}
```

**Result**: Archivist can't parse our WantList entries correctly.

#### 3. **Missing BlockPresence Address Field**

**Archivist:**
```protobuf
message BlockPresence {
  BlockAddress address = 1;  // Complex BlockAddress
  BlockPresenceType type = 2;
  bytes price = 3;
}
```

**Neverust:**
```protobuf
message BlockPresence {
  bytes cid = 1;  // Simple CID only
  BlockPresenceType type = 2;
  bytes price = 3;
}
```

---

## 📊 Comparison Matrix

| Component | Archivist-Node | Neverust | Status |
|-----------|----------------|----------|--------|
| **Message.payload type** | `Vec<BlockDelivery>` | `Vec<Block>` | ❌ **INCOMPATIBLE** |
| **WantlistEntry.address** | `BlockAddress` (complex) | `Vec<u8>` (simple) | ❌ **INCOMPATIBLE** |
| **BlockPresence.address** | `BlockAddress` (complex) | `Vec<u8>` (simple) | ❌ **INCOMPATIBLE** |
| **BlockDelivery structure** | Full support | Not implemented | ❌ **MISSING** |
| **BlockAddress structure** | Full support | Not implemented | ❌ **MISSING** |
| **ArchivistProof** | Full support | Not implemented | ❌ **MISSING** |
| **Merkle tree support** | Full support | Not implemented | ❌ **MISSING** |
| **Range retrieval fields** | Not supported | Custom extension | ⚠️ **INCOMPATIBLE** |

---

## 🔧 Required Fixes

### Priority 1: Message Format Compatibility

1. **Add BlockDelivery type** (replaces Block in payload)
2. **Add BlockAddress type** (complex CID + tree addressing)
3. **Add ArchivistProof type** (Merkle proofs)
4. **Update Message.payload**: `Vec<Block>` → `Vec<BlockDelivery>`
5. **Update WantlistEntry.address**: `Vec<u8>` → `BlockAddress`
6. **Update BlockPresence.address**: `Vec<u8>` → `BlockAddress`

### Priority 2: Protocol Flow

From Agent 9 (Request/Response Flow), the correct flow is:

**Client side:**
1. Send WantHave (check if peer has block)
2. Receive BlockPresence (peer says Have/DontHave)
3. Send WantBlock (request full block)
4. Receive BlockDelivery (with block data + proof if leaf)

**Neverust currently:**
1. ❌ Skips WantHave step
2. ❌ Immediately sends WantBlock
3. ❌ Uses wrong message format
4. ❌ Connection closes before response

### Priority 3: Mplex Configuration

**Status**: ✅ Already fixed in p2p.rs

The Mplex configuration was updated but didn't resolve the issue because the **message format incompatibility** causes immediate disconnection before any timeout would occur.

---

## 🎯 Implementation Plan

### Phase 1: Add Missing Types (1-2 hours)

**File**: `neverust-core/src/messages.rs`

```rust
// Add BlockAddress structure
#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockAddress {
    #[prost(bool, tag = "1")]
    pub leaf: bool,

    #[prost(bytes = "vec", tag = "2")]
    pub tree_cid: Vec<u8>,

    #[prost(uint64, tag = "3")]
    pub index: u64,

    #[prost(bytes = "vec", tag = "4")]
    pub cid: Vec<u8>,
}

// Add ArchivistProof structure
#[derive(Clone, PartialEq, prost::Message)]
pub struct ArchivistProof {
    #[prost(uint64, tag = "1")]
    pub mcodec: u64,

    #[prost(uint64, tag = "2")]
    pub index: u64,

    #[prost(uint64, tag = "3")]
    pub nleaves: u64,

    #[prost(message, repeated, tag = "4")]
    pub path: Vec<ProofNode>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ProofNode {
    #[prost(bytes = "vec", tag = "1")]
    pub hash: Vec<u8>,
}

// Add BlockDelivery structure
#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockDelivery {
    #[prost(bytes = "vec", tag = "1")]
    pub cid: Vec<u8>,

    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,

    #[prost(message, optional, tag = "3")]
    pub address: Option<BlockAddress>,

    #[prost(message, optional, tag = "4")]
    pub proof: Option<ArchivistProof>,
}

// Update Message payload field
#[derive(Clone, PartialEq, prost::Message)]
pub struct Message {
    #[prost(message, optional, tag = "1")]
    pub wantlist: Option<Wantlist>,

    #[prost(message, repeated, tag = "3")]  // Changed from Vec<Block>
    pub payload: Vec<BlockDelivery>,  // ← FIX: Use BlockDelivery

    #[prost(message, repeated, tag = "4")]
    pub block_presences: Vec<BlockPresence>,

    // ... rest unchanged
}

// Update WantlistEntry
#[derive(Clone, PartialEq, prost::Message)]
pub struct WantlistEntry {
    #[prost(message, optional, tag = "1")]  // Changed from bytes
    pub address: Option<BlockAddress>,  // ← FIX: Use BlockAddress

    #[prost(int32, tag = "2")]
    pub priority: i32,

    #[prost(bool, tag = "3")]
    pub cancel: bool,

    #[prost(enumeration = "WantType", tag = "4")]
    pub want_type: i32,

    #[prost(bool, tag = "5")]
    pub send_dont_have: bool,

    // REMOVE fields 6-7 (Neverust range extension)
    // These break compatibility with Archivist
}

// Update BlockPresence
#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockPresence {
    #[prost(message, optional, tag = "1")]  // Changed from bytes
    pub address: Option<BlockAddress>,  // ← FIX: Use BlockAddress

    #[prost(enumeration = "BlockPresenceType", tag = "2")]
    pub r#type: i32,

    #[prost(bytes = "vec", tag = "3")]
    pub price: Vec<u8>,
}
```

### Phase 2: Update BlockExc Logic (2-3 hours)

**File**: `neverust-core/src/blockexc.rs`

1. Update `request_block()` to send WantHave first
2. Handle BlockPresence responses
3. Only send WantBlock after confirming peer has the block
4. Parse BlockDelivery responses correctly
5. Validate Merkle proofs for leaf blocks

### Phase 3: Testing (1 hour)

1. Compile and fix any type errors
2. Run testnet integration test
3. Verify connections stay open
4. Verify block retrieval succeeds

**Total estimated time**: 4-6 hours

---

## 📝 Success Criteria

After implementing these fixes, we should see:

✅ Connections establish and stay open
✅ Protocol negotiation completes
✅ WantList messages accepted
✅ BlockDelivery responses received
✅ Blocks successfully retrieved and validated
✅ No UnexpectedEof errors

---

## 🔍 Agent Analysis Summary

All 10 analysis agents completed successfully:

1. **Transport Configuration**: Confirmed TCP+Noise+Mplex match
2. **BlockExc Protocol**: Identified message format differences
3. **Message Encoding**: Discovered BlockDelivery vs Block mismatch
4. **Connection Lifecycle**: Found 5-minute timeout requirements
5. **Protocol Negotiation**: Verified multistream-select compatibility
6. **Mplex Configuration**: Documented timeout parameters
7. **Noise Authentication**: Confirmed XX pattern compatibility
8. **Discovery & Bootstrap**: Identified missing UDP discovery layer
9. **Request/Response Flow**: Documented WantHave → WantBlock flow
10. **Error Handling**: Analyzed retry and recovery mechanisms

**Critical finding**: Message format incompatibility is the root cause, NOT transport configuration.

---

## 🚀 Next Steps

1. ✅ Mplex timeout fix applied (p2p.rs)
2. ⏳ **Implement message format fixes** (messages.rs) - IN PROGRESS
3. ⏳ Update BlockExc logic to use new types
4. ⏳ Test against Archivist testnet
5. ⏳ Add UDP discovery layer (future work)
6. ⏳ Implement full Merkle proof validation

**Estimated completion**: 1-2 days for core compatibility, 1-2 weeks for full feature parity.
