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
    pub payload: Vec<Block>,

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

#[derive(Clone, PartialEq,  prost::Message)]
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum WantType {
    WantBlock = 0,
    WantHave = 1,
}

#[derive(Clone, PartialEq,  prost::Message)]
pub struct Block {
    #[prost(bytes = "vec", tag = "1")]
    pub prefix: Vec<u8>,

    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,
}

#[derive(Clone, PartialEq,  prost::Message)]
pub struct BlockPresence {
    #[prost(bytes = "vec", tag = "1")]
    pub cid: Vec<u8>,

    #[prost(enumeration = "BlockPresenceType", tag = "2")]
    pub r#type: i32,

    #[prost(bytes = "vec", tag = "3")]
    pub price: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum BlockPresenceType {
    PresenceHave = 0,
    PresenceDontHave = 1,
}

#[derive(Clone, PartialEq,  prost::Message)]
pub struct AccountMessage {
    #[prost(bytes = "vec", tag = "1")]
    pub address: Vec<u8>,
}

#[derive(Clone, PartialEq,  prost::Message)]
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
                    block: vec![1, 2, 3, 4],
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
            decoded.wantlist.as_ref().unwrap().entries[0].block,
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn test_encode_decode_block() {
        let msg = Message {
            wantlist: None,
            payload: vec![Block {
                prefix: vec![0x12, 0x20], // sha256 multihash prefix
                data: vec![1, 2, 3, 4, 5],
            }],
            block_presences: vec![],
            pending_bytes: 0,
            account: None,
            payment: None,
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
        assert_eq!(decoded.payload.len(), 1);
        assert_eq!(decoded.payload[0].data, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_encode_decode_block_presence() {
        let msg = Message {
            wantlist: None,
            payload: vec![],
            block_presences: vec![BlockPresence {
                cid: vec![1, 2, 3],
                r#type: BlockPresenceType::PresenceHave as i32,
                price: vec![0; 32], // 32-byte UInt256
            }],
            pending_bytes: 0,
            account: None,
            payment: None,
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
        assert_eq!(decoded.block_presences.len(), 1);
        assert_eq!(decoded.block_presences[0].cid, vec![1, 2, 3]);
    }

    #[test]
    fn test_roundtrip_complex_message() {
        let msg = Message {
            wantlist: Some(Wantlist {
                entries: vec![
                    WantlistEntry {
                        block: vec![1, 2, 3],
                        priority: 1,
                        cancel: false,
                        want_type: WantType::WantBlock as i32,
                        send_dont_have: false,
                    },
                    WantlistEntry {
                        block: vec![4, 5, 6],
                        priority: 10,
                        cancel: true,
                        want_type: WantType::WantHave as i32,
                        send_dont_have: true,
                    },
                ],
                full: true,
            }),
            payload: vec![
                Block {
                    prefix: vec![0x12, 0x20],
                    data: vec![7, 8, 9],
                },
                Block {
                    prefix: vec![0x12, 0x20],
                    data: vec![10, 11, 12],
                },
            ],
            block_presences: vec![
                BlockPresence {
                    cid: vec![13, 14, 15],
                    r#type: BlockPresenceType::PresenceHave as i32,
                    price: vec![0; 32],
                },
                BlockPresence {
                    cid: vec![16, 17, 18],
                    r#type: BlockPresenceType::PresenceDontHave as i32,
                    price: vec![0; 32],
                },
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
}
