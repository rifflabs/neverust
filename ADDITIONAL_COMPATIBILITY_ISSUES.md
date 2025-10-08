# Additional Archivist Compatibility Issues Report

**Date**: 2025-10-08
**Scope**: Comprehensive analysis of ALL message format incompatibilities
**Status**: 🔍 **NEW ISSUES DISCOVERED**

---

## 🔍 Newly Discovered Incompatibilities

### 1. **Enum Value Naming Mismatch** ⚠️ **MEDIUM SEVERITY**

**Issue**: While enum integer values match, the **naming conventions differ** between .proto file and Nim implementation.

#### WantType Enum Discrepancy

**Archivist .proto file** (`message.proto`):
```protobuf
enum WantType {
  wantBlock = 0;    // camelCase
  wantHave = 1;     // camelCase
}
```

**Archivist Nim implementation** (`message.nim`):
```nim
type WantType* = enum
  WantBlock = 0    # PascalCase
  WantHave = 1     # PascalCase
```

**Neverust implementation** (`messages.rs`):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum WantType {
    WantBlock = 0,  // PascalCase
    WantHave = 1,   // PascalCase
}
```

**Impact**: ✅ **NO RUNTIME IMPACT** - Enum values are serialized as integers (0, 1), so this is purely cosmetic. However, it shows inconsistency in the codebase.

#### BlockPresenceType Enum Discrepancy

**Archivist .proto file** (`message.proto`):
```protobuf
enum BlockPresenceType {
  presenceHave = 0;        // camelCase with "presence" prefix
  presenceDontHave = 1;    // camelCase with "presence" prefix
}
```

**Archivist Nim implementation** (`message.nim`):
```nim
type BlockPresenceType* = enum
  Have = 0       # No prefix! Completely different!
  DontHave = 1   # No prefix! Completely different!
```

**Neverust implementation** (`messages.rs`):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum BlockPresenceType {
    PresenceHave = 0,      // PascalCase with "Presence" prefix
    PresenceDontHave = 1,  // PascalCase with "Presence" prefix
}
```

**Impact**: ✅ **NO RUNTIME IMPACT** - Same as above, values are serialized as integers. But Neverust follows .proto naming more closely than Archivist's own Nim implementation!

---

### 2. **Message.payload Field Number Skipped** ℹ️ **INFORMATIONAL**

**Observation**: Both implementations use `tag = "3"` for `payload` field, skipping `tag = "2"`.

**Archivist .proto**:
```protobuf
message Message {
  Wantlist wantlist = 1;
  repeated Block payload = 3;      // ← tag 2 is skipped
  repeated BlockPresence blockPresences = 4;
  int32 pendingBytes = 5;
  // ...
}
```

**Neverust implementation** (messages.rs):
```rust
pub struct Message {
    #[prost(message, optional, tag = "1")]
    pub wantlist: Option<Wantlist>,

    #[prost(message, repeated, tag = "3")]  // ← tag 2 is skipped
    pub payload: Vec<Block>,

    #[prost(message, repeated, tag = "4")]
    pub block_presences: Vec<BlockPresence>,
    // ...
}
```

**Impact**: ✅ **NO ISSUE** - This is intentional and common in protobuf (field 2 was likely deprecated). Both implementations match.

---

### 3. **WantlistEntry Field Name Mismatch** ⚠️ **MEDIUM SEVERITY - ALREADY DOCUMENTED**

This was already identified in the main report, but let me clarify the exact field structure:

**Archivist .proto** (SIMPLIFIED - doesn't match actual implementation!):
```protobuf
message Entry {
  bytes block = 1;       // the block cid
  int32 priority = 2;    // the priority (normalized). default to 1
  bool cancel = 3;       // whether this revokes an entry
  WantType wantType = 4; // Note: defaults to enum 0, ie Block
  bool sendDontHave = 5; // Note: defaults to false
}
```

**Archivist ACTUAL Nim implementation** (uses BlockAddress, NOT simple bytes!):
```nim
type WantListEntry* = object
  address*: BlockAddress     # ← NOT bytes! Complex structure!
  priority*: int32
  cancel*: bool
  wantType*: WantType
  sendDontHave*: bool
  inFlight*: bool            # Not serialized
```

**CRITICAL FINDING**: The `.proto` file in Archivist's repo is **OUT OF DATE** and does NOT match the actual Nim implementation! This is a documentation bug in archivist-node.

---

### 4. **BlockPresence Field Name Mismatch** ⚠️ **MEDIUM SEVERITY - ALREADY DOCUMENTED**

Same issue as WantlistEntry:

**Archivist .proto** (OUT OF DATE):
```protobuf
message BlockPresence {
  bytes cid = 1;
  BlockPresenceType type = 2;
  bytes price = 3;
}
```

**Archivist ACTUAL Nim implementation**:
```nim
type BlockPresence* = object
  address*: BlockAddress     # ← NOT bytes cid!
  `type`*: BlockPresenceType
  price*: seq[byte]
```

**Neverust implementation** (matches .proto, NOT Nim):
```rust
pub struct BlockPresence {
    #[prost(bytes = "vec", tag = "1")]
    pub cid: Vec<u8>,  // ← Matches .proto, not Nim implementation

    #[prost(enumeration = "BlockPresenceType", tag = "2")]
    pub r#type: i32,

    #[prost(bytes = "vec", tag = "3")]
    pub price: Vec<u8>,
}
```

**Impact**: ❌ **CRITICAL INCOMPATIBILITY** - This is the root cause of UnexpectedEof errors.

---

### 5. **Neverust Range Extension Fields** ⚠️ **HIGH SEVERITY - BREAKS COMPATIBILITY**

Neverust adds custom fields to `WantlistEntry` and `Block` for range retrieval that **do not exist** in Archivist.

#### WantlistEntry Range Fields

**Neverust** (`messages.rs`):
```rust
pub struct WantlistEntry {
    #[prost(bytes = "vec", tag = "1")]
    pub block: Vec<u8>,

    #[prost(int32, tag = "2")]
    pub priority: i32,

    #[prost(bool, tag = "3")]
    pub cancel: bool,

    #[prost(enumeration = "WantType", tag = "4")]
    pub want_type: i32,

    #[prost(bool, tag = "5")]
    pub send_dont_have: bool,

    // ⚠️ NEVERUST EXTENSION - NOT IN ARCHIVIST!
    #[prost(uint64, tag = "6")]
    pub start_byte: u64,

    #[prost(uint64, tag = "7")]
    pub end_byte: u64,
}
```

**Archivist** (`message.nim`):
```nim
type WantListEntry* = object
  address*: BlockAddress
  priority*: int32
  cancel*: bool
  wantType*: WantType
  sendDontHave*: bool
  # NO fields 6-7!
```

**Impact**: ⚠️ **POTENTIAL ISSUE** - If Neverust sends messages with non-zero `start_byte`/`end_byte`, Archivist will ignore these fields (protobuf behavior for unknown fields). However, this could cause confusion if Neverust expects range responses but Archivist doesn't support them.

#### Block Range Fields

**Neverust** (`messages.rs`):
```rust
pub struct Block {
    #[prost(bytes = "vec", tag = "1")]
    pub prefix: Vec<u8>,

    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,

    // ⚠️ NEVERUST EXTENSION - NOT IN ARCHIVIST!
    #[prost(uint64, tag = "3")]
    pub range_start: u64,

    #[prost(uint64, tag = "4")]
    pub range_end: u64,

    #[prost(uint64, tag = "5")]
    pub total_size: u64,
}
```

**Archivist uses BlockDelivery instead**:
```nim
type BlockDelivery* = object
  blk*: Block               # Contains cid + data only
  address*: BlockAddress
  proof*: ?ArchivistProof
```

**Impact**: ❌ **INCOMPATIBLE** - Neverust's `Block` structure is completely different from Archivist's `BlockDelivery`. This is a critical mismatch.

---

### 6. **Missing BlockAddress Structure** ❌ **CRITICAL - ALREADY DOCUMENTED**

**Archivist BlockAddress** (complex discriminated union):
```nim
type BlockAddress* = object
  case leaf*: bool
  of true:
    treeCid* {.serialize.}: Cid
    index* {.serialize.}: Natural
  else:
    cid* {.serialize.}: Cid
```

**Protobuf encoding** (from `message.nim`):
```nim
proc write*(pb: var ProtoBuffer, field: int, value: BlockAddress) =
  var ipb = initProtoBuffer()
  ipb.write(1, value.leaf.uint)      # bool leaf
  if value.leaf:
    ipb.write(2, value.treeCid.data.buffer)  # bytes treeCid
    ipb.write(3, value.index.uint64)         # uint64 index
  else:
    ipb.write(4, value.cid.data.buffer)      # bytes cid
  ipb.finish()
  pb.write(field, ipb)
```

**Equivalent protobuf schema** (derived from Nim code):
```protobuf
message BlockAddress {
  bool leaf = 1;
  bytes treeCid = 2;  // Only present if leaf=true
  uint64 index = 3;   // Only present if leaf=true
  bytes cid = 4;      // Only present if leaf=false
}
```

**Neverust implementation**: ❌ **MISSING ENTIRELY**

**Impact**: ❌ **CRITICAL** - Without BlockAddress, Neverust cannot communicate with Archivist nodes.

---

### 7. **Missing ArchivistProof Structure** ❌ **CRITICAL - ALREADY DOCUMENTED**

**Archivist ArchivistProof** (Merkle tree proof):
```nim
type ArchivistProof* = ref object of ByteProof
  mcodec*: MultiCodec
```

**Protobuf encoding** (from `coders.nim`):
```nim
proc encode*(self: ArchivistProof): seq[byte] =
  var pb = initProtoBuffer()
  pb.write(1, self.mcodec.uint64)    # uint64 mcodec
  pb.write(2, self.index.uint64)     # uint64 index
  pb.write(3, self.nleaves.uint64)   # uint64 nleaves

  for node in self.path:
    var nodesPb = initProtoBuffer()
    nodesPb.write(1, node)           # bytes hash
    nodesPb.finish()
    pb.write(4, nodesPb)             # repeated ProofNode

  pb.finish
  pb.buffer
```

**Equivalent protobuf schema** (derived from Nim code):
```protobuf
message ProofNode {
  bytes hash = 1;
}

message ArchivistProof {
  uint64 mcodec = 1;
  uint64 index = 2;
  uint64 nleaves = 3;
  repeated ProofNode path = 4;
}
```

**Neverust implementation**: ❌ **MISSING ENTIRELY**

**Impact**: ❌ **CRITICAL** - Required for verifying Merkle tree leaf blocks. Neverust cannot validate proofs from Archivist.

---

### 8. **Missing BlockDelivery Structure** ❌ **CRITICAL - ALREADY DOCUMENTED**

**Archivist BlockDelivery** (replaces simple Block):
```nim
type BlockDelivery* = object
  blk*: Block               # Contains cid + data
  address*: BlockAddress
  proof*: ?ArchivistProof   # Optional, only for leaves
```

**Protobuf encoding** (from `message.nim`):
```nim
proc write*(pb: var ProtoBuffer, field: int, value: BlockDelivery) =
  var ipb = initProtoBuffer()
  ipb.write(1, value.blk.cid.data.buffer)  # bytes cid
  ipb.write(2, value.blk.data)             # bytes data
  ipb.write(3, value.address)              # BlockAddress
  if value.address.leaf:
    if proof =? value.proof:
      ipb.write(4, proof.encode())         # ArchivistProof (optional)
  ipb.finish()
  pb.write(field, ipb)
```

**Equivalent protobuf schema** (derived from Nim code):
```protobuf
message BlockDelivery {
  bytes cid = 1;
  bytes data = 2;
  BlockAddress address = 3;
  bytes proof = 4;  // Optional, encoded ArchivistProof
}
```

**Neverust Message.payload field**:
```rust
pub struct Message {
    // ...
    #[prost(message, repeated, tag = "3")]
    pub payload: Vec<Block>,  // ← WRONG TYPE! Should be Vec<BlockDelivery>
    // ...
}
```

**Impact**: ❌ **CRITICAL** - This is the PRIMARY cause of UnexpectedEof errors. Archivist expects BlockDelivery but receives Block.

---

### 9. **pendingBytes Field Type Discrepancy** ⚠️ **LOW SEVERITY**

**Archivist .proto**:
```protobuf
message Message {
  // ...
  int32 pendingBytes = 5;
}
```

**Archivist Nim implementation**:
```nim
type Message* = object
  # ...
  pendingBytes*: uint  # ← uint, NOT int32!
```

**Neverust implementation**:
```rust
pub struct Message {
    // ...
    #[prost(int32, tag = "5")]
    pub pending_bytes: i32,  // ← Matches .proto, not Nim!
}
```

**Impact**: ⚠️ **MINOR** - `int32` and `uint` can both represent values 0-2147483647. Since `pendingBytes` should never be negative, this is likely a Nim implementation quirk. Neverust's choice of `i32` matches the .proto file and is safer.

---

## 📊 Comprehensive Compatibility Matrix

| Component | Archivist .proto | Archivist Nim | Neverust | Compatibility |
|-----------|-----------------|---------------|----------|---------------|
| **WantType enum values** | 0, 1 | 0, 1 | 0, 1 | ✅ **COMPATIBLE** |
| **WantType enum names** | camelCase | PascalCase | PascalCase | ⚠️ Cosmetic only |
| **BlockPresenceType values** | 0, 1 | 0, 1 | 0, 1 | ✅ **COMPATIBLE** |
| **BlockPresenceType names** | presenceHave | Have | PresenceHave | ⚠️ Cosmetic only |
| **WantlistEntry.address type** | bytes (WRONG!) | BlockAddress | bytes | ❌ **INCOMPATIBLE** |
| **WantlistEntry fields 6-7** | Not defined | Not defined | Range extension | ⚠️ Forward compatible |
| **Block structure** | prefix + data | cid + data | prefix + data + ranges | ❌ **INCOMPATIBLE** |
| **BlockDelivery** | Not in .proto | Fully implemented | Not implemented | ❌ **MISSING** |
| **BlockAddress** | Not in .proto | Fully implemented | Not implemented | ❌ **MISSING** |
| **ArchivistProof** | Not in .proto | Fully implemented | Not implemented | ❌ **MISSING** |
| **BlockPresence.address** | bytes cid (WRONG!) | BlockAddress | bytes cid | ❌ **INCOMPATIBLE** |
| **Message.payload type** | Block (WRONG!) | BlockDelivery | Block | ❌ **INCOMPATIBLE** |
| **pendingBytes type** | int32 | uint | int32 | ✅ **COMPATIBLE** |
| **Message field tag 2** | (skipped) | (skipped) | (skipped) | ✅ **COMPATIBLE** |

---

## ⚠️ Severity Assessment

### Critical Issues (Block All Communication)
1. ❌ **Message.payload type mismatch** (Block vs BlockDelivery)
2. ❌ **WantlistEntry.address type mismatch** (bytes vs BlockAddress)
3. ❌ **BlockPresence.address type mismatch** (bytes vs BlockAddress)
4. ❌ **Missing BlockDelivery structure**
5. ❌ **Missing BlockAddress structure**
6. ❌ **Missing ArchivistProof structure**

### High Priority Issues (Break Features)
7. ⚠️ **Neverust range extension fields** (breaks assumptions about message structure)

### Medium Priority Issues (Documentation/Clarity)
8. ⚠️ **Archivist .proto file is out of date** (doesn't match Nim implementation)
9. ⚠️ **Enum naming inconsistencies** (cosmetic but confusing)

### Low Priority Issues (Non-blocking)
10. ℹ️ **pendingBytes type discrepancy** (functionally equivalent)
11. ℹ️ **Field tag 2 skipped** (intentional, both match)

---

## 📝 Recommended Fixes

### For Neverust (Immediate - Phase 1)

#### 1. Add Missing Structures

**File**: `/opt/castle/workspace/neverust/neverust-core/src/messages.rs`

```rust
// Add ProofNode structure
#[derive(Clone, PartialEq, prost::Message)]
pub struct ProofNode {
    #[prost(bytes = "vec", tag = "1")]
    pub hash: Vec<u8>,
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

// Add BlockDelivery structure
#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockDelivery {
    #[prost(bytes = "vec", tag = "1")]
    pub cid: Vec<u8>,

    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,

    #[prost(message, optional, tag = "3")]
    pub address: Option<BlockAddress>,

    #[prost(bytes = "vec", optional, tag = "4")]
    pub proof: Option<Vec<u8>>,  // Encoded ArchivistProof
}
```

#### 2. Update Message Structure

```rust
#[derive(Clone, PartialEq, prost::Message)]
pub struct Message {
    #[prost(message, optional, tag = "1")]
    pub wantlist: Option<Wantlist>,

    // CHANGE: payload type from Vec<Block> to Vec<BlockDelivery>
    #[prost(message, repeated, tag = "3")]
    pub payload: Vec<BlockDelivery>,  // ← FIX

    #[prost(message, repeated, tag = "4")]
    pub block_presences: Vec<BlockPresence>,

    #[prost(int32, tag = "5")]
    pub pending_bytes: i32,

    #[prost(message, optional, tag = "6")]
    pub account: Option<AccountMessage>,

    #[prost(message, optional, tag = "7")]
    pub payment: Option<StateChannelUpdate>,
}
```

#### 3. Update WantlistEntry Structure

```rust
#[derive(Clone, PartialEq, prost::Message)]
pub struct WantlistEntry {
    // CHANGE: from bytes to BlockAddress
    #[prost(message, optional, tag = "1")]
    pub address: Option<BlockAddress>,  // ← FIX

    #[prost(int32, tag = "2")]
    pub priority: i32,

    #[prost(bool, tag = "3")]
    pub cancel: bool,

    #[prost(enumeration = "WantType", tag = "4")]
    pub want_type: i32,

    #[prost(bool, tag = "5")]
    pub send_dont_have: bool,

    // REMOVE: Neverust range extension (incompatible with Archivist)
    // #[prost(uint64, tag = "6")]
    // pub start_byte: u64,
    //
    // #[prost(uint64, tag = "7")]
    // pub end_byte: u64,
}
```

#### 4. Update BlockPresence Structure

```rust
#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockPresence {
    // CHANGE: from bytes cid to BlockAddress
    #[prost(message, optional, tag = "1")]
    pub address: Option<BlockAddress>,  // ← FIX

    #[prost(enumeration = "BlockPresenceType", tag = "2")]
    pub r#type: i32,

    #[prost(bytes = "vec", tag = "3")]
    pub price: Vec<u8>,
}
```

#### 5. Deprecate Old Block Structure

```rust
// DEPRECATED: Replaced by BlockDelivery
// Keep for backward compatibility with old tests
#[deprecated(note = "Use BlockDelivery instead")]
#[derive(Clone, PartialEq, prost::Message)]
pub struct Block {
    #[prost(bytes = "vec", tag = "1")]
    pub prefix: Vec<u8>,

    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,

    // Remove range extension fields
}
```

### For Archivist-Node (Upstream Bug Report)

#### Submit Issue: ".proto file is out of date"

**Title**: `message.proto` does not match Nim implementation

**Description**:
The `archivist/blockexchange/protobuf/message.proto` file is outdated and does not reflect the actual message structures used by the Nim implementation.

**Discrepancies**:
1. `WantListEntry.block` should be `WantListEntry.address` with type `BlockAddress`
2. `BlockPresence.cid` should be `BlockPresence.address` with type `BlockAddress`
3. `Message.payload` should use `BlockDelivery` type, not `Block`
4. Missing `BlockAddress` message definition
5. Missing `BlockDelivery` message definition
6. Missing `ArchivistProof` message definition (embedded in BlockDelivery)

**Impact**: Third-party implementations following the .proto file will be incompatible with Archivist nodes.

**Suggested Fix**: Update `.proto` file to match the Nim implementation, or generate `.proto` from Nim types automatically.

---

## ✅ Confirmation of Already-Documented Issues

The following issues were **correctly identified** in the original `ARCHIVIST_COMPATIBILITY_REPORT.md`:

1. ✅ **Payload field type mismatch** (Block vs BlockDelivery)
2. ✅ **WantlistEntry field structure** (bytes vs BlockAddress)
3. ✅ **Missing BlockPresence address field** (bytes vs BlockAddress)
4. ✅ **Missing BlockDelivery structure**
5. ✅ **Missing BlockAddress structure**
6. ✅ **Missing ArchivistProof structure**
7. ✅ **Merkle tree support missing**
8. ✅ **Range retrieval fields incompatibility**

**All critical issues were already documented.** This report adds:
- Detailed protobuf schema definitions
- Enum naming discrepancies (cosmetic)
- pendingBytes type discrepancy (minor)
- Confirmation that Archivist's .proto file is out of date
- Detailed encoding analysis from Nim source code

---

## 🎯 Next Steps

### Immediate (1-2 hours)
1. ✅ Read all source files (completed)
2. ✅ Generate detailed compatibility matrix (completed)
3. ⏳ Apply fixes to `messages.rs`
4. ⏳ Update all tests to use new structures

### Short Term (2-4 hours)
5. ⏳ Update `blockexc.rs` to build BlockDelivery messages
6. ⏳ Implement BlockAddress helper functions
7. ⏳ Add ArchivistProof validation (basic)
8. ⏳ Test against Archivist testnet

### Medium Term (1-2 weeks)
9. ⏳ Implement full Merkle tree proof verification
10. ⏳ Add support for tree CID addressing
11. ⏳ Consider re-adding range retrieval as optional extension

### Long Term (Future)
12. ⏳ Submit PR to archivist-node to update .proto file
13. ⏳ Propose range retrieval extension to Archivist protocol
14. ⏳ Implement automatic .proto generation from Rust types

---

## 📚 References

- **Archivist Protobuf**: `/tmp/archivist-node/archivist/blockexchange/protobuf/message.proto`
- **Archivist Nim Implementation**: `/tmp/archivist-node/archivist/blockexchange/protobuf/message.nim`
- **Archivist BlockAddress**: `/tmp/archivist-node/archivist/blocktype.nim`
- **Archivist Proof Encoding**: `/tmp/archivist-node/archivist/merkletree/archivist/coders.nim`
- **Neverust Messages**: `/opt/castle/workspace/neverust/neverust-core/src/messages.rs`
- **Original Report**: `/opt/castle/workspace/neverust/ARCHIVIST_COMPATIBILITY_REPORT.md`

---

**Report Complete** ✨
