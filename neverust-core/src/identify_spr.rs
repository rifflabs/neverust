//! SPR (Signed Peer Record) encoding shim for nim-libp2p v1.9.0 compatibility
//!
//! rust-libp2p 0.56's SPR encoding is incompatible with nim-libp2p v1.9.0.
//! This shim re-encodes SPRs to match nim-libp2p's expected format while
//! preserving all rust-libp2p functionality.
//!
//! The issue is in the Envelope protobuf encoding. While domain and payload type
//! match between implementations, the actual wire format differs.

use libp2p::{identity::Keypair, Multiaddr, PeerId};
use prost::Message;
use std::time::{SystemTime, UNIX_EPOCH};

/// PeerRecord protobuf message matching nim-libp2p's format
///
/// Protobuf definition:
/// ```protobuf
/// message PeerRecord {
///   message AddressInfo {
///     bytes multiaddr = 1;
///   }
///   bytes peer_id = 1;
///   uint64 seq = 2;
///   repeated AddressInfo addresses = 3;
/// }
/// ```
#[derive(Clone, PartialEq, Message)]
struct PeerRecord {
    /// Peer ID bytes
    #[prost(bytes = "vec", tag = "1")]
    peer_id: Vec<u8>,

    /// Sequence number (usually Unix timestamp)
    #[prost(uint64, tag = "2")]
    seq: u64,

    /// List of multiaddrs
    #[prost(message, repeated, tag = "3")]
    addresses: Vec<AddressInfo>,
}

#[derive(Clone, PartialEq, Message)]
struct AddressInfo {
    #[prost(bytes = "vec", tag = "1")]
    multiaddr: Vec<u8>,
}

/// Envelope protobuf message matching nim-libp2p's format
///
/// Protobuf definition:
/// ```protobuf
/// message Envelope {
///   bytes public_key = 1;
///   bytes payload_type = 2;
///   bytes payload = 3;
///   bytes signature = 5;  // Note: field 4 is skipped
/// }
/// ```
#[derive(Clone, PartialEq, Message)]
struct Envelope {
    /// Public key protobuf (field 1: KeyType, field 2: key bytes)
    #[prost(bytes = "vec", tag = "1")]
    public_key: Vec<u8>,

    /// Payload type multicodec: [0x03, 0x01] for libp2p-peer-record
    #[prost(bytes = "vec", tag = "2")]
    payload_type: Vec<u8>,

    /// Encoded PeerRecord
    #[prost(bytes = "vec", tag = "3")]
    payload: Vec<u8>,

    /// Signature over domain + payload_type + payload
    #[prost(bytes = "vec", tag = "5")]
    signature: Vec<u8>,
}

/// Domain string for peer records (must match nim-libp2p)
const PEER_RECORD_DOMAIN: &str = "libp2p-peer-record";

/// Payload type multicodec for peer records
const PEER_RECORD_PAYLOAD_TYPE: &[u8] = &[0x03, 0x01];

/// Create a signed peer record envelope compatible with nim-libp2p v1.9.0
///
/// This matches the exact encoding nim-libp2p expects:
/// 1. PeerRecord with peer_id, seq, and addresses
/// 2. Signature over: domain_len + domain + payload_type_len + payload_type + payload_len + payload
/// 3. Envelope with public_key, payload_type, payload, signature
pub fn create_signed_peer_record(
    keypair: &Keypair,
    peer_id: PeerId,
    addrs: Vec<Multiaddr>,
) -> Result<Vec<u8>, String> {
    // 1. Create PeerRecord
    let seq = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("System time error: {}", e))?
        .as_secs();

    let peer_record = PeerRecord {
        peer_id: peer_id.to_bytes(),
        seq,
        addresses: addrs
            .into_iter()
            .map(|addr| AddressInfo {
                multiaddr: addr.to_vec(),
            })
            .collect(),
    };

    // 2. Encode PeerRecord payload
    let mut payload = Vec::new();
    peer_record
        .encode(&mut payload)
        .map_err(|e| format!("Failed to encode PeerRecord: {}", e))?;

    // 3. Create signature buffer matching nim-libp2p's format
    // Concatenate: domain_len + domain + payload_type_len + payload_type + payload_len + payload
    let mut signature_buffer = Vec::new();

    // Write lengths as unsigned varint (matching nim-libp2p's VBuffer)
    write_varint(&mut signature_buffer, PEER_RECORD_DOMAIN.len() as u64);
    signature_buffer.extend_from_slice(PEER_RECORD_DOMAIN.as_bytes());

    write_varint(&mut signature_buffer, PEER_RECORD_PAYLOAD_TYPE.len() as u64);
    signature_buffer.extend_from_slice(PEER_RECORD_PAYLOAD_TYPE);

    write_varint(&mut signature_buffer, payload.len() as u64);
    signature_buffer.extend_from_slice(&payload);

    // 4. Sign the buffer
    let signature = keypair
        .sign(&signature_buffer)
        .map_err(|e| format!("Failed to sign: {}", e))?;

    // 5. Encode public key in protobuf format (field 1: KeyType, field 2: key bytes)
    let public_key_bytes = encode_public_key_protobuf(keypair)?;

    // 6. Create Envelope
    let envelope = Envelope {
        public_key: public_key_bytes,
        payload_type: PEER_RECORD_PAYLOAD_TYPE.to_vec(),
        payload,
        signature: signature.to_vec(),
    };

    // 7. Encode Envelope
    let mut envelope_bytes = Vec::new();
    envelope
        .encode(&mut envelope_bytes)
        .map_err(|e| format!("Failed to encode Envelope: {}", e))?;

    Ok(envelope_bytes)
}

/// Encode public key in protobuf format matching nim-libp2p
///
/// Protobuf definition:
/// ```protobuf
/// message PublicKey {
///   KeyType key_type = 1;  // enum: RSA=0, Ed25519=1, Secp256k1=2, ECDSA=3
///   bytes data = 2;
/// }
/// ```
fn encode_public_key_protobuf(keypair: &Keypair) -> Result<Vec<u8>, String> {
    // Use libp2p's built-in protobuf encoding
    // This ensures 100% compatibility with nim-libp2p's expectations
    let public_key = keypair.public();
    Ok(public_key.encode_protobuf())
}

/// Write unsigned varint (matching multiformats uvarint spec)
fn write_varint(buf: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_encoding() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 0);
        assert_eq!(buf, vec![0x00]);

        let mut buf = Vec::new();
        write_varint(&mut buf, 127);
        assert_eq!(buf, vec![0x7F]);

        let mut buf = Vec::new();
        write_varint(&mut buf, 128);
        assert_eq!(buf, vec![0x80, 0x01]);

        let mut buf = Vec::new();
        write_varint(&mut buf, 300);
        assert_eq!(buf, vec![0xAC, 0x02]);
    }

    #[test]
    fn test_create_signed_peer_record() {
        let keypair = Keypair::generate_secp256k1();
        let peer_id = PeerId::from(keypair.public());
        let addrs = vec![
            "/ip4/127.0.0.1/tcp/8070".parse().unwrap(),
        ];

        let result = create_signed_peer_record(&keypair, peer_id, addrs);
        assert!(result.is_ok());

        let envelope_bytes = result.unwrap();
        assert!(envelope_bytes.len() > 0);

        // Verify it decodes as valid protobuf
        let envelope = Envelope::decode(&envelope_bytes[..]);
        assert!(envelope.is_ok());
    }
}
