//! SPR (Signed Peer Record) parsing
//!
//! Decodes base64-encoded SPR records into multiaddrs for bootstrap nodes

use libp2p::{core::{SignedEnvelope, PeerRecord}, Multiaddr, PeerId};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SprError {
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("SignedEnvelope decode error: {0}")]
    Envelope(String),

    #[error("PeerRecord decode error: {0}")]
    PeerRecord(String),
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

    // Decode URL-safe base64 (SPR records use URL-safe encoding)
    let bytes = URL_SAFE_NO_PAD.decode(spr_base64)?;

    // Decode SignedEnvelope from protobuf
    let signed_envelope = SignedEnvelope::from_protobuf_encoding(&bytes)
        .map_err(|e| SprError::Envelope(e.to_string()))?;

    // Decode PeerRecord from SignedEnvelope
    let peer_record = PeerRecord::from_signed_envelope(signed_envelope)
        .map_err(|e| SprError::PeerRecord(e.to_string()))?;

    // Extract peer ID and addresses
    let peer_id = peer_record.peer_id();
    let addrs = peer_record.addresses().to_vec();

    Ok((peer_id, addrs))
}

#[cfg(test)]
mod tests {
    use super::*;

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
