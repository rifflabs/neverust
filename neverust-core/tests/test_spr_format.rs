// Test to examine the SPR (Signed Peer Record) format that rust-libp2p generates
// This will help us compare with nim-libp2p's expectations

use libp2p::{
    identity::Keypair,
    PeerId,
};

#[test]
fn test_examine_spr_bytes() {
    // Create a secp256k1 keypair (matching our production config)
    let keypair = Keypair::generate_secp256k1();
    let peer_id = PeerId::from(keypair.public());

    println!("\n=== SPR Format Test ===");
    println!("Peer ID: {}", peer_id);
    println!("Key type: secp256k1");

    // Create a PeerRecord with some test addresses
    let addrs = vec![
        "/ip4/127.0.0.1/tcp/8070".parse().unwrap(),
        "/ip4/10.7.1.200/tcp/8070".parse().unwrap(),
    ];

    // Create a signed peer record
    match libp2p::core::signed_envelope::PeerRecord::new(&keypair, addrs.clone()) {
        Ok(peer_record) => {
            println!("\nPeerRecord created successfully");

            // Try to access the envelope
            // Note: PeerRecord wraps a SignedEnvelope
            // Let's see if we can get the raw bytes

            // The PeerRecord type in rust-libp2p has methods to convert to bytes
            if let Ok(envelope_bytes) = peer_record.to_signed_envelope().into_protobuf_encoding() {
                println!("\nSigned Envelope bytes (len={}): ", envelope_bytes.len());
                print_hex(&envelope_bytes);

                // Also print the payload separately
                println!("\nPayload bytes:");
                // The payload is the encoded PeerRecord protobuf
                // which contains: peerId (field 1), seq (field 2), addrs (field 3)
            }
        }
        Err(e) => {
            println!("Failed to create PeerRecord: {:?}", e);
        }
    }
}

fn print_hex(bytes: &[u8]) {
    for (i, byte) in bytes.iter().enumerate() {
        if i % 16 == 0 {
            if i > 0 {
                println!();
            }
            print!("{:04x}: ", i);
        }
        print!("{:02x} ", byte);
    }
    println!();
}
