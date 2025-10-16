//! Example demonstrating the Advertiser engine
//!
//! This example shows how to:
//! 1. Set up Discovery and Advertiser
//! 2. Announce blocks to the DHT
//! 3. Automatic re-advertisement
//!
//! Run with:
//! ```bash
//! cargo run --example advertiser_example
//! ```

use neverust_core::{Advertiser, Discovery};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Advertiser Engine Example ===\n");

    // Step 1: Create discovery service
    println!("1. Creating Discovery service...");
    let keypair = libp2p::identity::Keypair::generate_secp256k1();
    let listen_addr = "127.0.0.1:9000".parse()?;
    let announce_addrs = vec!["/ip4/127.0.0.1/tcp/8070".to_string()];

    let discovery = Arc::new(Discovery::new(&keypair, listen_addr, announce_addrs, vec![]).await?);

    println!("   ✓ Discovery service created");
    println!("   Local Peer ID: {}\n", discovery.local_peer_id());

    // Step 2: Create advertiser with custom settings
    println!("2. Creating Advertiser engine...");
    let advertiser = Advertiser::new(
        Arc::clone(&discovery),
        10,                           // Max 10 concurrent announcements
        Duration::from_secs(30 * 60), // Re-advertise every 30 minutes
    );

    println!("   ✓ Advertiser created");
    println!("   Max concurrent: 10");
    println!("   Re-advertise interval: 30 minutes\n");

    // Step 3: Start the advertiser
    println!("3. Starting advertiser engine...");
    advertiser.start().await?;
    println!("   ✓ Advertiser started\n");

    // Step 4: Announce some blocks
    println!("4. Announcing blocks to the DHT...");
    let test_cids = vec![
        "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
        "bafybeie5gq4jxvzmsym6hjlwxej4rwdoxt7wadqvmmwbqi7r27fclha2va",
        "bafybeihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku",
    ];

    for cid_str in test_cids {
        let cid: neverust_core::Cid = cid_str.parse()?;
        advertiser.advertise_block(&cid).await?;
        println!("   ✓ Queued: {}", cid);
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    println!(
        "\n   In-flight announcements: {}",
        advertiser.in_flight_count().await
    );

    // Step 5: Demonstrate re-advertisement
    println!("\n5. Re-advertisement demonstration...");
    println!("   (In production, blocks are re-announced every 30 minutes)");
    println!("   This keeps them discoverable in the DHT");

    // Step 6: Simulate storing a new block and auto-announcing
    println!("\n6. Automatic announcement on block storage...");
    println!("   When integrated with BlockStore:");
    println!("   - Store new block → BlockStore.put()");
    println!("   - Callback triggered → on_block_stored()");
    println!("   - Auto-queued → Advertiser.advertise_block()");
    println!("   - Announced to DHT → Discovery.provide()");

    // Step 7: Clean shutdown
    println!("\n7. Stopping advertiser...");
    advertiser.stop().await;
    println!("   ✓ Advertiser stopped cleanly\n");

    println!("=== Example Complete ===");
    println!("\nKey Features:");
    println!("  • Queue-based announcement system");
    println!("  • Concurrent request limiting (default: 10)");
    println!("  • Periodic re-advertisement (default: 30 min)");
    println!("  • Lifecycle management (start/stop)");
    println!("  • Integration with BlockStore callbacks");

    Ok(())
}
