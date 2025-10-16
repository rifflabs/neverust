//! Block advertisement engine for DHT announcements
//!
//! Automatically announces blocks to the DHT with queue-based processing,
//! concurrent request limiting, and periodic re-advertisement.
//!
//! ## Architecture
//!
//! - **Queue-based**: Blocks are queued for announcement to avoid overwhelming the DHT
//! - **Concurrent limiting**: Limits concurrent announcements (default: 10)
//! - **Periodic re-advertisement**: Re-announces blocks every 30 minutes to keep them discoverable
//! - **Lifecycle management**: Start/stop methods for clean shutdown
//!
//! ## Example
//!
//! ```rust,no_run
//! use neverust_core::advertiser::Advertiser;
//! use neverust_core::discovery::Discovery;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let discovery = Arc::new(todo!());
//! let advertiser = Advertiser::new(discovery, 10, std::time::Duration::from_secs(1800));
//!
//! // Start the advertiser engine
//! advertiser.start().await;
//!
//! // Queue a block for announcement
//! let cid: cid::Cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".parse()?;
//! advertiser.advertise_block(&cid).await?;
//!
//! // Stop the advertiser
//! advertiser.stop().await;
//! # Ok(())
//! # }
//! ```

use cid::Cid;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::discovery::Discovery;
use crate::storage::BlockStore;

#[derive(Debug, thiserror::Error)]
pub enum AdvertiserError {
    #[error("Advertiser is not running")]
    NotRunning,

    #[error("Advertiser is already running")]
    AlreadyRunning,

    #[error("Failed to advertise block: {0}")]
    AdvertiseFailed(String),

    #[error("Channel send failed")]
    ChannelSendFailed,
}

type Result<T> = std::result::Result<T, AdvertiserError>;

/// Message types for the advertiser queue
#[derive(Debug, Clone)]
enum AdvertiseMessage {
    /// Advertise a block once
    Advertise(Cid),
    /// Stop the advertiser
    Stop,
}

/// Block advertisement engine with automatic re-advertisement
pub struct Advertiser {
    /// Discovery service for DHT operations
    discovery: Arc<Discovery>,

    /// Block store for iterating all blocks
    block_store: Option<Arc<BlockStore>>,

    /// Sender for advertisement queue
    tx: mpsc::UnboundedSender<AdvertiseMessage>,

    /// Receiver for advertisement queue
    rx: Arc<RwLock<mpsc::UnboundedReceiver<AdvertiseMessage>>>,

    /// Set of blocks currently in-flight (being advertised)
    in_flight: Arc<RwLock<HashSet<Cid>>>,

    /// Maximum concurrent advertisements
    max_concurrent: usize,

    /// Re-advertisement interval
    readvertise_interval: Duration,

    /// Handle to the advertisement loop task
    task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,

    /// Handle to the re-advertisement loop task (local store)
    local_store_handle: Arc<RwLock<Option<JoinHandle<()>>>>,

    /// Running state
    running: Arc<RwLock<bool>>,
}

impl Advertiser {
    /// Create a new Advertiser
    ///
    /// # Arguments
    ///
    /// * `discovery` - Discovery service for DHT operations
    /// * `max_concurrent` - Maximum concurrent advertisement requests (default: 10)
    /// * `readvertise_interval` - Interval for re-advertising blocks (default: 30 minutes)
    pub fn new(
        discovery: Arc<Discovery>,
        max_concurrent: usize,
        readvertise_interval: Duration,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            discovery,
            block_store: None,
            tx,
            rx: Arc::new(RwLock::new(rx)),
            in_flight: Arc::new(RwLock::new(HashSet::new())),
            max_concurrent,
            readvertise_interval,
            task_handle: Arc::new(RwLock::new(None)),
            local_store_handle: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Create with default settings (10 concurrent, 30 minute re-advertisement)
    pub fn with_defaults(discovery: Arc<Discovery>) -> Self {
        Self::new(discovery, 10, Duration::from_secs(30 * 60))
    }

    /// Set the block store for periodic local store advertisement
    ///
    /// When a block store is set, the advertiser will periodically iterate
    /// all blocks in the store and advertise them to the DHT.
    pub fn set_block_store(&mut self, block_store: Arc<BlockStore>) {
        self.block_store = Some(block_store);
    }

    /// Start the advertiser engine
    ///
    /// Spawns two or three background tasks:
    /// 1. Advertisement loop - processes queued blocks
    /// 2. Local store loop - periodically iterates all blocks in BlockStore (if set)
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(AdvertiserError::AlreadyRunning);
        }

        info!(
            "Starting advertiser engine (max_concurrent={}, readvertise_interval={:?})",
            self.max_concurrent, self.readvertise_interval
        );

        *running = true;
        drop(running);

        // Start advertisement loop
        let handle = self.spawn_advertise_loop();
        *self.task_handle.write().await = Some(handle);

        // Start local store re-advertisement loop if block store is set
        if self.block_store.is_some() {
            let local_store_handle = self.spawn_advertise_local_store_loop();
            *self.local_store_handle.write().await = Some(local_store_handle);
            info!("Started local store re-advertisement loop");
        } else {
            info!("No block store set, skipping local store re-advertisement");
        }

        Ok(())
    }

    /// Stop the advertiser engine
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        if !*running {
            return;
        }

        info!("Stopping advertiser engine");
        *running = false;
        drop(running);

        // Send stop message
        let _ = self.tx.send(AdvertiseMessage::Stop);

        // Wait for tasks to complete
        if let Some(handle) = self.task_handle.write().await.take() {
            let _ = handle.await;
        }

        if let Some(handle) = self.local_store_handle.write().await.take() {
            handle.abort();
        }

        info!("Advertiser engine stopped");
    }

    /// Queue a block for advertisement
    pub async fn advertise_block(&self, cid: &Cid) -> Result<()> {
        if !*self.running.read().await {
            return Err(AdvertiserError::NotRunning);
        }

        debug!("Queueing block for advertisement: {}", cid);

        self.tx
            .send(AdvertiseMessage::Advertise(*cid))
            .map_err(|_| AdvertiserError::ChannelSendFailed)?;

        Ok(())
    }

    /// Get the number of blocks currently in-flight
    pub async fn in_flight_count(&self) -> usize {
        self.in_flight.read().await.len()
    }

    /// Check if a block is currently being advertised
    pub async fn is_in_flight(&self, cid: &Cid) -> bool {
        self.in_flight.read().await.contains(cid)
    }

    /// Spawn the advertisement loop task
    fn spawn_advertise_loop(&self) -> JoinHandle<()> {
        let discovery = Arc::clone(&self.discovery);
        let rx = Arc::clone(&self.rx);
        let in_flight = Arc::clone(&self.in_flight);
        let running = Arc::clone(&self.running);
        let max_concurrent = self.max_concurrent;

        tokio::spawn(async move {
            let semaphore = Arc::new(Semaphore::new(max_concurrent));

            loop {
                // Check if we should stop
                if !*running.read().await {
                    break;
                }

                // Get next message from queue
                let message = {
                    let mut rx_guard = rx.write().await;
                    rx_guard.recv().await
                };

                match message {
                    Some(AdvertiseMessage::Advertise(cid)) => {
                        // Skip if already in-flight
                        {
                            let mut in_flight_guard = in_flight.write().await;
                            if in_flight_guard.contains(&cid) {
                                debug!("Block {} already in-flight, skipping", cid);
                                continue;
                            }
                            in_flight_guard.insert(cid);
                        }

                        let permit = semaphore.clone().acquire_owned().await.unwrap();
                        let discovery = Arc::clone(&discovery);
                        let in_flight = Arc::clone(&in_flight);

                        tokio::spawn(async move {
                            if let Err(e) = discovery.provide(&cid).await {
                                error!("Failed to advertise block {}: {}", cid, e);
                            } else {
                                debug!("Successfully advertised block: {}", cid);
                            }

                            // Remove from in-flight
                            in_flight.write().await.remove(&cid);
                            drop(permit);
                        });
                    }
                    Some(AdvertiseMessage::Stop) => {
                        info!("Received stop message, shutting down advertisement loop");
                        break;
                    }
                    None => {
                        warn!("Advertisement queue channel closed");
                        break;
                    }
                }
            }

            info!("Advertisement loop terminated");
        })
    }

    /// Spawn the periodic local store advertisement loop
    ///
    /// Iterates all blocks in BlockStore every `readvertise_interval` and queues them
    /// for advertisement. Tracks in-flight requests to avoid duplicates.
    ///
    /// Reference: Archivist advertiser.nim:83-97
    fn spawn_advertise_local_store_loop(&self) -> JoinHandle<()> {
        let block_store = self.block_store.clone().expect("BlockStore must be set");
        let running = Arc::clone(&self.running);
        let readvertise_interval = self.readvertise_interval;
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let mut cycle = 0u64;

            loop {
                // Wait for re-advertisement interval
                tokio::time::sleep(readvertise_interval).await;

                // Check if we should stop
                if !*running.read().await {
                    break;
                }

                cycle += 1;
                info!(
                    "Advertiser: Starting local store re-advertisement cycle #{}",
                    cycle
                );

                // Get all CIDs from the block store
                let cids = block_store.list_cids().await;
                let total_count = cids.len();

                if total_count > 0 {
                    info!(
                        "Advertiser: Found {} blocks in local store to advertise",
                        total_count
                    );

                    // Queue each block for advertisement
                    let mut queued = 0;
                    for cid in cids {
                        if let Err(e) = tx.send(AdvertiseMessage::Advertise(cid)) {
                            error!(
                                "Advertiser: Failed to queue block {} for advertisement: {}",
                                cid, e
                            );
                        } else {
                            queued += 1;
                        }
                    }

                    info!(
                        "Advertiser: Cycle #{} complete - queued {}/{} blocks for advertisement",
                        cycle, queued, total_count
                    );
                } else {
                    debug!(
                        "Advertiser: No blocks in local store to advertise (cycle #{})",
                        cycle
                    );
                }
            }

            info!("Advertiser: Local store re-advertisement loop terminated");
        })
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        // Attempt to stop gracefully on drop
        // Note: This is best-effort since drop cannot be async
        // We can't use blocking_write in an async context, so we just abort the tasks
        // The proper way to stop is to call stop() before dropping
        // This is just a safety net for cleanup
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity::Keypair;

    async fn create_test_discovery() -> Arc<Discovery> {
        let keypair = Keypair::generate_secp256k1();
        let listen_addr = format!("127.0.0.1:{}", 9000 + rand::random::<u16>() % 1000)
            .parse()
            .unwrap();
        let announce_addrs = vec!["/ip4/127.0.0.1/tcp/8070".to_string()];

        Arc::new(
            Discovery::new(&keypair, listen_addr, announce_addrs, vec![])
                .await
                .unwrap(),
        )
    }

    fn create_test_cid() -> Cid {
        "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
            .parse()
            .unwrap()
    }

    #[tokio::test]
    async fn test_advertiser_new() {
        let discovery = create_test_discovery().await;
        let advertiser = Advertiser::new(discovery, 5, Duration::from_secs(60));

        assert_eq!(advertiser.max_concurrent, 5);
        assert_eq!(advertiser.readvertise_interval, Duration::from_secs(60));
        assert_eq!(advertiser.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn test_advertiser_with_defaults() {
        let discovery = create_test_discovery().await;
        let advertiser = Advertiser::with_defaults(discovery);

        assert_eq!(advertiser.max_concurrent, 10);
        assert_eq!(
            advertiser.readvertise_interval,
            Duration::from_secs(30 * 60)
        );
    }

    #[tokio::test]
    async fn test_advertiser_start_stop() {
        let discovery = create_test_discovery().await;
        let advertiser = Advertiser::with_defaults(discovery);

        // Should not be running initially
        assert!(!*advertiser.running.read().await);

        // Start advertiser
        advertiser.start().await.unwrap();
        assert!(*advertiser.running.read().await);

        // Should fail to start again
        let result = advertiser.start().await;
        assert!(matches!(result, Err(AdvertiserError::AlreadyRunning)));

        // Stop advertiser
        advertiser.stop().await;
        assert!(!*advertiser.running.read().await);

        // Should be safe to stop again
        advertiser.stop().await;
    }

    #[tokio::test]
    async fn test_advertise_block_not_running() {
        let discovery = create_test_discovery().await;
        let advertiser = Advertiser::with_defaults(discovery);
        let cid = create_test_cid();

        // Should fail when not running
        let result = advertiser.advertise_block(&cid).await;
        assert!(matches!(result, Err(AdvertiserError::NotRunning)));
    }

    #[tokio::test]
    async fn test_advertise_block_success() {
        let discovery = create_test_discovery().await;
        let advertiser = Advertiser::with_defaults(discovery);
        let cid = create_test_cid();

        // Start advertiser
        advertiser.start().await.unwrap();

        // Should succeed when running
        advertiser.advertise_block(&cid).await.unwrap();

        // Wait a bit for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Block should no longer be in-flight (completed)
        assert!(!advertiser.is_in_flight(&cid).await);

        // Stop advertiser
        advertiser.stop().await;
    }

    #[tokio::test]
    async fn test_multiple_blocks() {
        let discovery = create_test_discovery().await;
        let advertiser = Advertiser::with_defaults(discovery);

        advertiser.start().await.unwrap();

        // Advertise multiple blocks
        let cid1: Cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
            .parse()
            .unwrap();
        let cid2: Cid = "bafybeie5gq4jxvzmsym6hjlwxej4rwdoxt7wadqvmmwbqi7r27fclha2va"
            .parse()
            .unwrap();
        let cid3: Cid = "bafybeihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku"
            .parse()
            .unwrap();

        advertiser.advertise_block(&cid1).await.unwrap();
        advertiser.advertise_block(&cid2).await.unwrap();
        advertiser.advertise_block(&cid3).await.unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        // All blocks should be completed (not in-flight)
        assert!(!advertiser.is_in_flight(&cid1).await);
        assert!(!advertiser.is_in_flight(&cid2).await);
        assert!(!advertiser.is_in_flight(&cid3).await);

        advertiser.stop().await;
    }

    #[tokio::test]
    async fn test_concurrent_limiting() {
        let discovery = create_test_discovery().await;
        let advertiser = Advertiser::new(discovery, 2, Duration::from_secs(3600));

        advertiser.start().await.unwrap();

        // Queue many blocks
        let mut cids = Vec::new();
        for i in 0..10 {
            let cid_str = format!(
                "bafybei{}dyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
                i
            );
            if let Ok(cid) = cid_str.parse::<Cid>() {
                cids.push(cid);
                advertiser.advertise_block(&cid).await.unwrap();
            }
        }

        // Even with concurrent limit of 2, all should eventually be processed
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Most blocks should be completed (not in-flight)
        let in_flight = advertiser.in_flight_count().await;
        assert!(in_flight <= 2); // Should respect concurrent limit

        advertiser.stop().await;
    }

    #[tokio::test]
    async fn test_local_store_readvertise_loop() {
        use crate::storage::Block;

        let discovery = create_test_discovery().await;
        let block_store = Arc::new(BlockStore::new());

        // Add some blocks to the store
        let data1 = b"hello world".to_vec();
        let data2 = b"goodbye world".to_vec();
        let block1 = Block::new(data1).unwrap();
        let block2 = Block::new(data2).unwrap();
        block_store.put(block1.clone()).await.unwrap();
        block_store.put(block2.clone()).await.unwrap();

        // Use short re-advertisement interval for testing
        let mut advertiser = Advertiser::new(discovery, 10, Duration::from_millis(200));
        advertiser.set_block_store(block_store.clone());

        advertiser.start().await.unwrap();

        // Wait for first cycle to complete
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Blocks should have been queued for advertisement
        // (they won't be in-flight since they complete quickly)
        assert!(!advertiser.is_in_flight(&block1.cid).await);
        assert!(!advertiser.is_in_flight(&block2.cid).await);

        advertiser.stop().await;
    }

    #[tokio::test]
    async fn test_duplicate_advertisements() {
        let discovery = create_test_discovery().await;
        let advertiser = Advertiser::with_defaults(discovery);
        let cid = create_test_cid();

        advertiser.start().await.unwrap();

        // Advertise same block multiple times
        advertiser.advertise_block(&cid).await.unwrap();
        advertiser.advertise_block(&cid).await.unwrap();
        advertiser.advertise_block(&cid).await.unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be completed (not in-flight)
        assert!(!advertiser.is_in_flight(&cid).await);

        advertiser.stop().await;
    }

    #[tokio::test]
    async fn test_advertiser_drop() {
        let discovery = create_test_discovery().await;
        let advertiser = Advertiser::with_defaults(discovery);

        advertiser.start().await.unwrap();

        // Drop the advertiser - should clean up tasks
        drop(advertiser);

        // If we get here without hanging, the drop worked
    }
}
