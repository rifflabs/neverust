//! Neverust - Archivist Storage Node in Rust
//!
//! A high-performance P2P storage node implementation using rust-libp2p.

use neverust_core::{Config, run_node};
use std::error::Error;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Parse CLI arguments and build config
    let config = Config::from_cli()?;

    // Initialize logging
    init_logging(&config.log_level);

    tracing::info!("Starting Neverust node...");

    // Run the node
    run_node(config).await?;

    Ok(())
}

fn init_logging(level: &str) {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(level))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
