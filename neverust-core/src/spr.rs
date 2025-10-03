//! SPR (Signed Peer Record) parsing
//!
//! Decodes base64-encoded SPR records into multiaddrs for bootstrap nodes.
//!
//! ## Format Investigation
//!
//! Archivist's SPR format is a libp2p `SignedEnvelope` containing a custom peer record:
//! - Base64 URL-safe encoded protobuf
//! - Contains `SignedEnvelope` with peer public key and signature
//! - Payload contains peer information including multiaddrs
//!
//! The exact payload structure differs from standard libp2p `PeerRecord`.
//! For now, we attempt standard `PeerRecord` decoding, which works for the
//! peer ID but may fail for multiaddr extraction with Archivist's custom format.
//!
//! ## Known Issues
//! - Archivist uses custom protobuf structure for multiaddrs
//! - Standard `PeerRecord::from_signed_envelope` may fail with "payload extraction error"
//! - Alternative: manual protobuf parsing of the envelope payload
//!
//! ## References
//! - Archivist testnet SPR endpoint: https://spr.archivist.storage/testnet
//! - libp2p SignedEnvelope: https://github.com/libp2p/specs/blob/master/RFC/0003-routing-records.md

use libp2p::{Multiaddr, PeerId};
use prost::Message;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SprError {
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("Protobuf decode error: {0}")]
    Protobuf(#[from] prost::DecodeError),

    #[error("Invalid peer ID: {0}")]
    InvalidPeerId(String),

    #[error("Invalid multiaddr: {0}")]
    InvalidMultiaddr(String),
}

/// PeerInfo protobuf message
#[derive(Clone, PartialEq, Message)]
struct PeerInfo {
    #[prost(bytes = "vec", tag = "1")]
    peer_id: Vec<u8>,
    #[prost(bytes = "vec", repeated, tag = "2")]
    multiaddrs: Vec<Vec<u8>>,
}

/// Parse SPR records from testnet endpoint response
pub fn parse_spr_records(spr_text: &str) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, SprError> {
    let mut results = Vec::new();

    for line in spr_text.lines() {
        if let Some(spr_data) = line.strip_prefix("spr:") {
            match parse_single_spr(spr_data) {
                Ok((peer_id, addrs)) => {
                    tracing::info!("Parsed SPR: peer_id={}, addrs={:?}", peer_id, addrs);
                    results.push((peer_id, addrs));
                }
                Err(e) => {
                    tracing::warn!("Failed to parse SPR record: {}", e);
                    continue;
                }
            }
        }
    }

    Ok(results)
}

/// Parse a single base64-encoded SPR record
fn parse_single_spr(spr_base64: &str) -> Result<(PeerId, Vec<Multiaddr>), SprError> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use libp2p::core::{SignedEnvelope, PeerRecord};

    // Decode URL-safe base64 (SPR records use URL-safe encoding)
    let bytes = URL_SAFE_NO_PAD.decode(spr_base64)?;

    // Decode SignedEnvelope from protobuf
    let signed_envelope = SignedEnvelope::from_protobuf_encoding(&bytes)
        .map_err(|e| SprError::Protobuf(prost::DecodeError::new(e.to_string())))?;

    // Try to decode PeerRecord from SignedEnvelope
    let peer_record = PeerRecord::from_signed_envelope(signed_envelope)
        .map_err(|e| SprError::InvalidPeerId(e.to_string()))?;

    // Extract peer ID and addresses
    let peer_id = peer_record.peer_id();
    let addrs = peer_record.addresses().to_vec();

    Ok((peer_id, addrs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    #[test]
    fn test_inspect_spr_as_envelope() {
        use libp2p::core::SignedEnvelope;

        let spr_text = "spr:CiUIAhIhA5mg11LZgFQ4XzIRb1T5xw9muFW1ALNKTijyKhQmvKYXEgIDARpJCicAJQgCEiEDmaDXUtmAVDhfMhFvVPnHD2a4VbUAs0pOKPIqFCa8phcQl-XFxQYaCwoJBE4vqKqRAnU6GgsKCQROL6iqkQJ1OipHMEUCIQDfzVYbN6A_O4i29e_FtDDUo7GJS3bkXRQtoteYbPSFtgIgcc8Kgj2ggVJyK16EY9xi4bY2lpTTeNIRjvslXSRdN5w";

        if let Some(spr_data) = spr_text.strip_prefix("spr:") {
            let bytes = URL_SAFE_NO_PAD.decode(spr_data).unwrap();

            // Try to decode as SignedEnvelope
            match SignedEnvelope::from_protobuf_encoding(&bytes) {
                Ok(envelope) => {
                    println!("SignedEnvelope decoded successfully!");
                    println!("Payload type: {}", envelope.payload_type());
                    println!("Payload len: {} bytes", envelope.payload().len());
                    println!("Payload hex: {}", hex::encode(envelope.payload()));
                }
                Err(e) => {
                    println!("SignedEnvelope decode failed: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_parse_spr_records() {
        // Real SPR from Archivist testnet
        let spr_text = "spr:CiUIAhIhA5mg11LZgFQ4XzIRb1T5xw9muFW1ALNKTijyKhQmvKYXEgIDARpJCicAJQgCEiEDmaDXUtmAVDhfMhFvVPnHD2a4VbUAs0pOKPIqFCa8phcQl-XFxQYaCwoJBE4vqKqRAnU6GgsKCQROL6iqkQJ1OipHMEUCIQDfzVYbN6A_O4i29e_FtDDUo7GJS3bkXRQtoteYbPSFtgIgcc8Kgj2ggVJyK16EY9xi4bY2lpTTeNIRjvslXSRdN5w";

        // Try parsing and print any error
        if let Some(spr_data) = spr_text.strip_prefix("spr:") {
            match parse_single_spr(spr_data) {
                Ok((peer_id, addrs)) => {
                    println!("Success! Peer: {}, Addrs: {:?}", peer_id, addrs);
                    assert!(addrs.len() > 0);
                }
                Err(e) => {
                    panic!("Failed to parse SPR: {}", e);
                }
            }
        } else {
            panic!("No spr: prefix found");
        }
    }
}
