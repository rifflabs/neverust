//! BlockExc protobuf messages
//!
//! Manual implementation of protobuf messages from proto/message.proto
//! Using prost derive macros for encoding/decoding

use prost::Message as ProstMessage;

#[derive(Clone, PartialEq, prost::Message)]
pub struct Message {
    #[prost(message, optional, tag = "1")]
    pub wantlist: Option<Wantlist>,

    #[prost(message, repeated, tag = "3")]
    pub payload: Vec<BlockDelivery>,

    #[prost(message, repeated, tag = "4")]
    pub block_presences: Vec<BlockPresence>,

    #[prost(int32, tag = "5")]
    pub pending_bytes: i32,

    #[prost(message, optional, tag = "6")]
    pub account: Option<AccountMessage>,

    #[prost(message, optional, tag = "7")]
    pub payment: Option<StateChannelUpdate>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Wantlist {
    #[prost(message, repeated, tag = "1")]
    pub entries: Vec<WantlistEntry>,

    #[prost(bool, tag = "2")]
    pub full: bool,
}

/// BlockAddress represents a location in the content-addressed storage system.
/// It can reference either:
/// - A simple block by CID (leaf=false, cid set)
/// - A Merkle tree leaf (leaf=true, treeCid and index set)
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

impl BlockAddress {
    /// Create a simple CID-based BlockAddress (non-leaf)
    pub fn from_cid(cid: Vec<u8>) -> Self {
        Self {
            leaf: false,
            tree_cid: vec![],
            index: 0,
            cid,
        }
    }

    /// Create a Merkle tree leaf BlockAddress
    pub fn from_tree_leaf(tree_cid: Vec<u8>, index: u64) -> Self {
        Self {
            leaf: true,
            tree_cid,
            index,
            cid: vec![],
        }
    }

    /// Get the CID bytes (works for both simple CID and tree leaf)
    pub fn cid_bytes(&self) -> &[u8] {
        if self.leaf {
            &self.tree_cid
        } else {
            &self.cid
        }
    }
}

/// ProofNode represents a single node in a Merkle proof path
#[derive(Clone, PartialEq, prost::Message)]
pub struct ProofNode {
    #[prost(bytes = "vec", tag = "1")]
    pub hash: Vec<u8>,
}

/// ArchivistProof contains a Merkle proof for verifying block authenticity
#[derive(Clone, PartialEq, prost::Message)]
pub struct ArchivistProof {
    /// Multicodec identifier for the hash function used
    #[prost(uint64, tag = "1")]
    pub mcodec: u64,

    /// Index of this leaf in the Merkle tree
    #[prost(uint64, tag = "2")]
    pub index: u64,

    /// Total number of leaves in the Merkle tree
    #[prost(uint64, tag = "3")]
    pub nleaves: u64,

    /// Merkle proof path (sibling hashes from leaf to root)
    #[prost(message, repeated, tag = "4")]
    pub path: Vec<ProofNode>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct WantlistEntry {
    /// BlockAddress (complex structure supporting both simple CIDs and Merkle tree leaves)
    #[prost(message, optional, tag = "1")]
    pub address: Option<BlockAddress>,

    #[prost(int32, tag = "2")]
    pub priority: i32,

    #[prost(bool, tag = "3")]
    pub cancel: bool,

    #[prost(enumeration = "WantType", tag = "4")]
    pub want_type: i32,

    #[prost(bool, tag = "5")]
    pub send_dont_have: bool,
}

impl WantlistEntry {
    /// Create a WantlistEntry from a simple CID (non-Merkle tree)
    pub fn from_cid(cid: Vec<u8>, want_type: WantType) -> Self {
        Self {
            address: Some(BlockAddress::from_cid(cid)),
            priority: 1,
            cancel: false,
            want_type: want_type as i32,
            send_dont_have: true,
        }
    }

    /// Create a WantlistEntry from a CID struct (convenience method)
    pub fn from_cid_struct(cid: &cid::Cid, want_type: WantType) -> Self {
        Self::from_cid(cid.to_bytes(), want_type)
    }

    /// Create a WantlistEntry for a Merkle tree leaf
    pub fn from_tree_leaf(tree_cid: Vec<u8>, index: u64, want_type: WantType) -> Self {
        Self {
            address: Some(BlockAddress::from_tree_leaf(tree_cid, index)),
            priority: 1,
            cancel: false,
            want_type: want_type as i32,
            send_dont_have: true,
        }
    }

    /// Create a cancel entry for a given CID
    pub fn cancel_cid(cid: Vec<u8>) -> Self {
        Self {
            address: Some(BlockAddress::from_cid(cid)),
            priority: 0,
            cancel: true,
            want_type: WantType::WantBlock as i32,
            send_dont_have: false,
        }
    }

    /// Get the CID bytes from this entry's address
    pub fn cid_bytes(&self) -> Option<&[u8]> {
        self.address.as_ref().map(|addr| addr.cid_bytes())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum WantType {
    WantBlock = 0,
    WantHave = 1,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Block {
    #[prost(bytes = "vec", tag = "1")]
    pub prefix: Vec<u8>,

    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,

    // Neverust extension: Range response metadata for partial blocks
    #[prost(uint64, tag = "3")]
    pub range_start: u64,

    #[prost(uint64, tag = "4")]
    pub range_end: u64,

    #[prost(uint64, tag = "5")]
    pub total_size: u64,
}

/// BlockDelivery represents a complete block delivery with optional Merkle proof
/// This is the Archivist-compatible format sent in Message.payload
#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockDelivery {
    /// Block CID
    #[prost(bytes = "vec", tag = "1")]
    pub cid: Vec<u8>,

    /// Block data
    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,

    /// Block address (simple CID or Merkle tree leaf)
    #[prost(message, optional, tag = "3")]
    pub address: Option<BlockAddress>,

    /// Merkle proof (only present for Merkle tree leaves)
    #[prost(message, optional, tag = "4")]
    pub proof: Option<ArchivistProof>,
}

impl BlockDelivery {
    /// Create a BlockDelivery from a simple CID and data
    pub fn from_cid_and_data(cid: Vec<u8>, data: Vec<u8>) -> Self {
        Self {
            cid: cid.clone(),
            data,
            address: Some(BlockAddress::from_cid(cid)),
            proof: None,
        }
    }

    /// Create a BlockDelivery for a Merkle tree leaf with proof
    pub fn from_tree_leaf(
        cid: Vec<u8>,
        data: Vec<u8>,
        tree_cid: Vec<u8>,
        index: u64,
        proof: ArchivistProof,
    ) -> Self {
        Self {
            cid,
            data,
            address: Some(BlockAddress::from_tree_leaf(tree_cid, index)),
            proof: Some(proof),
        }
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockPresence {
    #[prost(message, optional, tag = "1")]
    pub address: Option<BlockAddress>,

    #[prost(enumeration = "BlockPresenceType", tag = "2")]
    pub r#type: i32,

    #[prost(bytes = "vec", tag = "3")]
    pub price: Vec<u8>,
}

impl BlockPresence {
    /// Create a BlockPresence from a simple CID
    pub fn from_cid(cid: Vec<u8>, presence_type: BlockPresenceType, price: Vec<u8>) -> Self {
        Self {
            address: Some(BlockAddress::from_cid(cid)),
            r#type: presence_type as i32,
            price,
        }
    }

    /// Get the CID bytes from this presence notification
    pub fn cid_bytes(&self) -> Option<&[u8]> {
        self.address.as_ref().map(|addr| addr.cid_bytes())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum BlockPresenceType {
    PresenceHave = 0,
    PresenceDontHave = 1,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AccountMessage {
    #[prost(bytes = "vec", tag = "1")]
    pub address: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StateChannelUpdate {
    #[prost(bytes = "vec", tag = "1")]
    pub update: Vec<u8>,
}

/// Encode a BlockExc message to bytes
pub fn encode_message(msg: &Message) -> Result<Vec<u8>, prost::EncodeError> {
    let mut buf = Vec::new();
    msg.encode(&mut buf)?;
    Ok(buf)
}

/// Decode a BlockExc message from bytes
pub fn decode_message(bytes: &[u8]) -> Result<Message, prost::DecodeError> {
    Message::decode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_empty_message() {
        let msg = Message {
            wantlist: None,
            payload: vec![],
            block_presences: vec![],
            pending_bytes: 0,
            account: None,
            payment: None,
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_encode_decode_wantlist() {
        let msg = Message {
            wantlist: Some(Wantlist {
                entries: vec![WantlistEntry {
                    address: Some(BlockAddress::from_cid(vec![1, 2, 3, 4])),
                    priority: 100,
                    cancel: false,
                    want_type: WantType::WantBlock as i32,
                    send_dont_have: false,
                }],
                full: false,
            }),
            payload: vec![],
            block_presences: vec![],
            pending_bytes: 0,
            account: None,
            payment: None,
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
        assert_eq!(decoded.wantlist.as_ref().unwrap().entries.len(), 1);
        assert_eq!(
            decoded.wantlist.as_ref().unwrap().entries[0]
                .cid_bytes()
                .unwrap(),
            &[1, 2, 3, 4]
        );
    }

    #[test]
    fn test_encode_decode_block() {
        let cid = vec![0x12, 0x20, 1, 2, 3, 4]; // sha256 multihash
        let data = vec![1, 2, 3, 4, 5];

        let msg = Message {
            wantlist: None,
            payload: vec![BlockDelivery::from_cid_and_data(cid.clone(), data.clone())],
            block_presences: vec![],
            pending_bytes: 0,
            account: None,
            payment: None,
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
        assert_eq!(decoded.payload.len(), 1);
        assert_eq!(decoded.payload[0].data, data);
        assert_eq!(decoded.payload[0].cid, cid);
    }

    #[test]
    fn test_encode_decode_block_presence() {
        let cid = vec![1, 2, 3];
        let msg = Message {
            wantlist: None,
            payload: vec![],
            block_presences: vec![BlockPresence::from_cid(
                cid.clone(),
                BlockPresenceType::PresenceHave,
                vec![0; 32], // 32-byte UInt256
            )],
            pending_bytes: 0,
            account: None,
            payment: None,
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
        assert_eq!(decoded.block_presences.len(), 1);
        assert_eq!(decoded.block_presences[0].cid_bytes().unwrap(), &cid[..]);
    }

    #[test]
    fn test_roundtrip_complex_message() {
        let msg = Message {
            wantlist: Some(Wantlist {
                entries: vec![
                    WantlistEntry {
                        address: Some(BlockAddress::from_cid(vec![1, 2, 3])),
                        priority: 1,
                        cancel: false,
                        want_type: WantType::WantBlock as i32,
                        send_dont_have: false,
                    },
                    WantlistEntry {
                        address: Some(BlockAddress::from_cid(vec![4, 5, 6])),
                        priority: 10,
                        cancel: true,
                        want_type: WantType::WantHave as i32,
                        send_dont_have: true,
                    },
                ],
                full: true,
            }),
            payload: vec![
                BlockDelivery::from_cid_and_data(vec![0x12, 0x20, 7, 8, 9], vec![7, 8, 9]),
                BlockDelivery::from_cid_and_data(vec![0x12, 0x20, 10, 11, 12], vec![10, 11, 12]),
            ],
            block_presences: vec![
                BlockPresence::from_cid(
                    vec![13, 14, 15],
                    BlockPresenceType::PresenceHave,
                    vec![0; 32],
                ),
                BlockPresence::from_cid(
                    vec![16, 17, 18],
                    BlockPresenceType::PresenceDontHave,
                    vec![0; 32],
                ),
            ],
            pending_bytes: 12345,
            account: Some(AccountMessage {
                address: vec![0xAA; 20], // Ethereum address
            }),
            payment: Some(StateChannelUpdate {
                update: b"signed_nitro_state_json".to_vec(),
            }),
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
        assert_eq!(decoded.wantlist.as_ref().unwrap().entries.len(), 2);
        assert_eq!(decoded.payload.len(), 2);
        assert_eq!(decoded.block_presences.len(), 2);
        assert_eq!(decoded.pending_bytes, 12345);
        assert!(decoded.account.is_some());
        assert!(decoded.payment.is_some());
    }

    #[test]
    fn test_block_address_simple_cid() {
        // Test simple CID-based BlockAddress
        let cid_bytes = vec![1u8, 2, 3, 4];
        let addr = BlockAddress::from_cid(cid_bytes.clone());

        assert!(!addr.leaf);
        assert_eq!(addr.cid, cid_bytes);
        assert_eq!(addr.tree_cid, Vec::<u8>::new());
        assert_eq!(addr.index, 0);
        assert_eq!(addr.cid_bytes(), &cid_bytes[..]);
    }

    #[test]
    fn test_block_address_tree_leaf() {
        // Test Merkle tree leaf BlockAddress
        let tree_cid = vec![5u8, 6, 7, 8];
        let index = 42;
        let addr = BlockAddress::from_tree_leaf(tree_cid.clone(), index);

        assert!(addr.leaf);
        assert_eq!(addr.tree_cid, tree_cid);
        assert_eq!(addr.index, index);
        assert_eq!(addr.cid, Vec::<u8>::new());
        assert_eq!(addr.cid_bytes(), &tree_cid[..]);
    }

    #[test]
    fn test_wantlist_entry_from_cid() {
        // Test creating WantlistEntry from CID
        let cid_bytes = vec![1, 2, 3, 4];
        let entry = WantlistEntry::from_cid(cid_bytes.clone(), WantType::WantBlock);

        assert!(entry.address.is_some());
        assert_eq!(entry.cid_bytes().unwrap(), &cid_bytes[..]);
        assert_eq!(entry.priority, 1);
        assert!(!entry.cancel);
        assert_eq!(entry.want_type, WantType::WantBlock as i32);
        assert!(entry.send_dont_have);
    }

    #[test]
    fn test_wantlist_entry_from_tree_leaf() {
        // Test creating WantlistEntry for Merkle tree leaf
        let tree_cid = vec![5, 6, 7, 8];
        let index = 42;
        let entry = WantlistEntry::from_tree_leaf(tree_cid.clone(), index, WantType::WantHave);

        assert!(entry.address.is_some());
        let addr = entry.address.as_ref().unwrap();
        assert!(addr.leaf);
        assert_eq!(addr.tree_cid, tree_cid);
        assert_eq!(addr.index, index);
        assert_eq!(entry.want_type, WantType::WantHave as i32);
    }

    #[test]
    fn test_wantlist_entry_cancel() {
        // Test creating cancel entry
        let cid_bytes = vec![1, 2, 3, 4];
        let entry = WantlistEntry::cancel_cid(cid_bytes.clone());

        assert!(entry.address.is_some());
        assert_eq!(entry.cid_bytes().unwrap(), &cid_bytes[..]);
        assert!(entry.cancel);
        assert_eq!(entry.priority, 0);
    }

    #[test]
    fn test_block_delivery_simple_cid() {
        // Test BlockDelivery with simple CID
        let cid = vec![1, 2, 3, 4];
        let data = vec![5, 6, 7, 8];
        let delivery = BlockDelivery::from_cid_and_data(cid.clone(), data.clone());

        assert_eq!(delivery.cid, cid);
        assert_eq!(delivery.data, data);
        assert!(delivery.address.is_some());
        assert!(delivery.proof.is_none());

        let addr = delivery.address.unwrap();
        assert!(!addr.leaf);
        assert_eq!(addr.cid, cid);
    }

    #[test]
    fn test_block_delivery_tree_leaf() {
        // Test BlockDelivery with Merkle tree leaf
        let cid = vec![1, 2, 3, 4];
        let data = vec![5, 6, 7, 8];
        let tree_cid = vec![9, 10, 11, 12];
        let index = 42;
        let proof = ArchivistProof {
            mcodec: 0x12,
            index,
            nleaves: 100,
            path: vec![ProofNode {
                hash: vec![13, 14, 15],
            }],
        };

        let delivery = BlockDelivery::from_tree_leaf(
            cid.clone(),
            data.clone(),
            tree_cid.clone(),
            index,
            proof.clone(),
        );

        assert_eq!(delivery.cid, cid);
        assert_eq!(delivery.data, data);
        assert!(delivery.address.is_some());
        assert!(delivery.proof.is_some());

        let addr = delivery.address.unwrap();
        assert!(addr.leaf);
        assert_eq!(addr.tree_cid, tree_cid);
        assert_eq!(addr.index, index);

        let returned_proof = delivery.proof.unwrap();
        assert_eq!(returned_proof.mcodec, 0x12);
        assert_eq!(returned_proof.nleaves, 100);
    }
}
