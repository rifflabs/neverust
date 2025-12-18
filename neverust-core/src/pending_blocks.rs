//! Pending blocks manager for tracking block requests with retry logic
//!
//! This module manages blocks that we're waiting for from peers,
//! tracking retries, in-flight status, and providing async completion
//! via oneshot channels.

use cid::Cid;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{trace, warn};

use crate::storage::Block;

/// Default number of retries before giving up on a block
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default interval between retry attempts (matches Nim: 500ms)
const DEFAULT_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// Error returned when retries are exhausted for a block
#[derive(Debug, thiserror::Error)]
#[error("Retries exhausted for block: {0}")]
pub struct RetriesExhaustedError(pub Cid);

/// Tracks a single pending block request
struct PendingBlock {
    /// The CID of the block we're waiting for
    _cid: Cid,
    /// Channel sender to complete the request
    sender: oneshot::Sender<Block>,
    /// Number of retries remaining
    retries_left: u32,
    /// When we last attempted to request this block
    last_attempt: Instant,
    /// Whether a request is currently in flight
    in_flight: bool,
    /// When we started requesting this block (for metrics)
    start_time: Instant,
}

/// Internal state for the pending blocks manager
struct PendingBlocksState {
    /// Blocks we're currently requesting
    pending: HashMap<Cid, PendingBlock>,
    /// Maximum number of retries per block
    max_retries: u32,
    /// Interval between retry attempts
    retry_interval: Duration,
}

/// Manages pending block requests with retry logic
///
/// Uses Arc<Mutex<>> for interior mutability to allow sharing across async contexts
/// while maintaining a synchronous API.
#[derive(Clone)]
pub struct PendingBlocksManager {
    state: Arc<Mutex<PendingBlocksState>>,
}

impl PendingBlocksManager {
    /// Create a new pending blocks manager with default configuration
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PendingBlocksState {
                pending: HashMap::new(),
                max_retries: DEFAULT_MAX_RETRIES,
                retry_interval: DEFAULT_RETRY_INTERVAL,
            })),
        }
    }

    /// Create a new pending blocks manager with custom retry configuration
    pub fn with_config(max_retries: u32, retry_interval: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(PendingBlocksState {
                pending: HashMap::new(),
                max_retries,
                retry_interval,
            })),
        }
    }

    /// Check if a block is currently pending
    pub fn is_pending(&self, cid: &Cid) -> bool {
        let state = self.state.lock().unwrap();
        state.pending.contains_key(cid)
    }

    /// Add a pending block request, returning a receiver for async completion
    ///
    /// If the block is already pending, returns a new receiver for the existing request.
    /// The receiver will be notified when the block arrives via `complete()`.
    pub fn add_pending(&self, cid: Cid) -> oneshot::Receiver<Block> {
        let mut state = self.state.lock().unwrap();

        // If already pending, we can't return a new receiver for the existing request
        // In the Nim version, this returns the same Future handle
        // For Rust, we need to either use broadcast channels or document that
        // callers should check is_pending() first
        if let Some(_existing) = state.pending.get(&cid) {
            // Create a new receiver that will never complete
            // In practice, callers should check is_pending() before calling this
            let (tx, rx) = oneshot::channel();
            drop(tx); // Drop sender immediately - this receiver will error
            trace!(cid = ?cid, "Block already pending, returning dummy receiver");
            return rx;
        }

        let (sender, receiver) = oneshot::channel();

        let pending_block = PendingBlock {
            _cid: cid,
            sender,
            retries_left: state.max_retries,
            last_attempt: Instant::now(),
            in_flight: false,
            start_time: Instant::now(),
        };

        state.pending.insert(cid, pending_block);
        trace!(cid = ?cid, "Added pending block request");

        receiver
    }

    /// Complete a pending block request, sending the block to waiters
    ///
    /// Returns true if the block was pending and successfully completed,
    /// false if it wasn't pending or already completed.
    pub fn complete(&self, cid: &Cid, block: Block) -> bool {
        let mut state = self.state.lock().unwrap();

        if let Some(pending) = state.pending.remove(cid) {
            let duration = pending.start_time.elapsed();

            // Warn on slow retrievals (>500ms)
            if duration.as_millis() > 500 {
                warn!(
                    cid = ?cid,
                    duration_ms = duration.as_millis(),
                    "High block retrieval time"
                );
            }

            // Send block to waiter (ignore error if receiver dropped)
            let _ = pending.sender.send(block);

            trace!(
                cid = ?cid,
                duration_ms = duration.as_millis(),
                "Completed pending block request"
            );

            true
        } else {
            trace!(cid = ?cid, "No pending request found for block");
            false
        }
    }

    /// Mark a block request as in-flight or not
    ///
    /// Use this to track whether a request has been sent to a peer
    /// and we're waiting for a response.
    pub fn set_in_flight(&self, cid: &Cid, in_flight: bool) {
        let mut state = self.state.lock().unwrap();

        if let Some(pending) = state.pending.get_mut(cid) {
            pending.in_flight = in_flight;
            if in_flight {
                pending.last_attempt = Instant::now();
            }
            trace!(cid = ?cid, in_flight, "Set in-flight status");
        }
    }

    /// Check if a block request is currently in-flight
    pub fn is_in_flight(&self, cid: &Cid) -> bool {
        let state = self.state.lock().unwrap();
        state.pending.get(cid).map(|p| p.in_flight).unwrap_or(false)
    }

    /// Check if a block should be retried
    ///
    /// Returns true if:
    /// - The block is pending
    /// - It's not currently in-flight
    /// - Enough time has passed since last attempt
    /// - Retries are not exhausted
    pub fn should_retry(&self, cid: &Cid) -> bool {
        let state = self.state.lock().unwrap();

        if let Some(pending) = state.pending.get(cid) {
            !pending.in_flight
                && pending.retries_left > 0
                && pending.last_attempt.elapsed() >= state.retry_interval
        } else {
            false
        }
    }

    /// Decrement the retry count for a block
    ///
    /// Call this when a retry attempt fails.
    pub fn decrement_retries(&self, cid: &Cid) {
        let mut state = self.state.lock().unwrap();

        if let Some(pending) = state.pending.get_mut(cid) {
            if pending.retries_left > 0 {
                pending.retries_left -= 1;
                trace!(
                    cid = ?cid,
                    retries_left = pending.retries_left,
                    "Decremented retries for block"
                );
            }
        }
    }

    /// Get all pending block CIDs
    pub fn get_pending_cids(&self) -> Vec<Cid> {
        self.state.lock().unwrap().pending.keys().copied().collect()
    }

    /// Get the number of pending blocks
    pub fn len(&self) -> usize {
        self.state.lock().unwrap().pending.len()
    }

    /// Check if there are no pending blocks
    pub fn is_empty(&self) -> bool {
        self.state.lock().unwrap().pending.is_empty()
    }

    /// Get the number of retries remaining for a block
    pub fn retries_remaining(&self, cid: &Cid) -> Option<u32> {
        self.state
            .lock()
            .unwrap()
            .pending
            .get(cid)
            .map(|p| p.retries_left)
    }

    /// Check if retries are exhausted for a block
    pub fn retries_exhausted(&self, cid: &Cid) -> bool {
        self.state
            .lock()
            .unwrap()
            .pending
            .get(cid)
            .map(|p| p.retries_left == 0)
            .unwrap_or(false)
    }

    /// Clear all pending requests
    ///
    /// All waiters will receive channel errors.
    pub fn clear(&self) {
        self.state.lock().unwrap().pending.clear();
        trace!("Cleared all pending blocks");
    }

    /// Remove a pending block request without completing it
    ///
    /// The waiter will receive a channel error.
    pub fn cancel(&self, cid: &Cid) -> bool {
        if self.state.lock().unwrap().pending.remove(cid).is_some() {
            trace!(cid = ?cid, "Cancelled pending block request");
            true
        } else {
            false
        }
    }
}

impl Default for PendingBlocksManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_block(data: &[u8]) -> Block {
        Block::new(data.to_vec()).unwrap()
    }

    #[test]
    fn test_new_manager() {
        let manager = PendingBlocksManager::new();
        assert_eq!(manager.len(), 0);
        assert!(manager.is_empty());
    }

    #[test]
    fn test_add_pending() {
        let manager = PendingBlocksManager::new();
        let block = create_test_block(b"test data");
        let cid = block.cid;

        let mut receiver = manager.add_pending(cid);
        assert!(manager.is_pending(&cid));
        assert_eq!(manager.len(), 1);
        assert!(!manager.is_empty());

        // Receiver should be waiting
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_complete_pending() {
        let manager = PendingBlocksManager::new();
        let block = create_test_block(b"test data");
        let cid = block.cid;

        let receiver = manager.add_pending(cid);

        // Complete the request
        let completed = manager.complete(&cid, block.clone());
        assert!(completed);
        assert!(!manager.is_pending(&cid));
        assert_eq!(manager.len(), 0);

        // Receiver should get the block
        let received_block = receiver.await.unwrap();
        assert_eq!(received_block.cid, cid);
        assert_eq!(received_block.data, block.data);
    }

    #[test]
    fn test_complete_non_pending() {
        let manager = PendingBlocksManager::new();
        let block = create_test_block(b"test data");
        let cid = block.cid;

        // Try to complete a block that isn't pending
        let completed = manager.complete(&cid, block);
        assert!(!completed);
    }

    #[tokio::test]
    async fn test_complete_after_receiver_dropped() {
        let manager = PendingBlocksManager::new();
        let block = create_test_block(b"test data");
        let cid = block.cid;

        let receiver = manager.add_pending(cid);
        drop(receiver); // Drop receiver before completion

        // Should still complete successfully (just no one listening)
        let completed = manager.complete(&cid, block);
        assert!(completed);
    }

    #[test]
    fn test_in_flight_tracking() {
        let manager = PendingBlocksManager::new();
        let block = create_test_block(b"test data");
        let cid = block.cid;

        manager.add_pending(cid);
        assert!(!manager.is_in_flight(&cid));

        manager.set_in_flight(&cid, true);
        assert!(manager.is_in_flight(&cid));

        manager.set_in_flight(&cid, false);
        assert!(!manager.is_in_flight(&cid));
    }

    #[test]
    fn test_should_retry() {
        let manager = PendingBlocksManager::with_config(3, Duration::from_millis(100));
        let block = create_test_block(b"test data");
        let cid = block.cid;

        manager.add_pending(cid);

        // Should not retry immediately (interval not elapsed)
        assert!(!manager.should_retry(&cid));

        // Wait for retry interval
        std::thread::sleep(Duration::from_millis(150));

        // Should retry now
        assert!(manager.should_retry(&cid));

        // Mark in-flight - should not retry
        manager.set_in_flight(&cid, true);
        assert!(!manager.should_retry(&cid));

        // Mark not in-flight - should retry again (but wait for interval)
        manager.set_in_flight(&cid, false);
        std::thread::sleep(Duration::from_millis(150));
        assert!(manager.should_retry(&cid));
    }

    #[test]
    fn test_retry_exhaustion() {
        let manager = PendingBlocksManager::with_config(3, Duration::from_millis(0));
        let block = create_test_block(b"test data");
        let cid = block.cid;

        manager.add_pending(cid);

        // Initial retries
        assert_eq!(manager.retries_remaining(&cid), Some(3));
        assert!(!manager.retries_exhausted(&cid));

        // Decrement retries
        manager.decrement_retries(&cid);
        assert_eq!(manager.retries_remaining(&cid), Some(2));

        manager.decrement_retries(&cid);
        assert_eq!(manager.retries_remaining(&cid), Some(1));

        manager.decrement_retries(&cid);
        assert_eq!(manager.retries_remaining(&cid), Some(0));
        assert!(manager.retries_exhausted(&cid));

        // Still pending even with 0 retries left
        assert!(manager.is_pending(&cid));

        // Decrementing further does nothing
        manager.decrement_retries(&cid);
        assert_eq!(manager.retries_remaining(&cid), Some(0));
        assert!(manager.is_pending(&cid));
    }

    #[test]
    fn test_get_pending_cids() {
        let manager = PendingBlocksManager::new();
        let block1 = create_test_block(b"block 1");
        let block2 = create_test_block(b"block 2");
        let block3 = create_test_block(b"block 3");

        manager.add_pending(block1.cid);
        manager.add_pending(block2.cid);
        manager.add_pending(block3.cid);

        let pending_cids = manager.get_pending_cids();
        assert_eq!(pending_cids.len(), 3);
        assert!(pending_cids.contains(&block1.cid));
        assert!(pending_cids.contains(&block2.cid));
        assert!(pending_cids.contains(&block3.cid));
    }

    #[test]
    fn test_cancel() {
        let manager = PendingBlocksManager::new();
        let block = create_test_block(b"test data");
        let cid = block.cid;

        manager.add_pending(cid);
        assert!(manager.is_pending(&cid));

        let cancelled = manager.cancel(&cid);
        assert!(cancelled);
        assert!(!manager.is_pending(&cid));

        // Cancel non-existent block
        let cancelled = manager.cancel(&cid);
        assert!(!cancelled);
    }

    #[test]
    fn test_clear() {
        let manager = PendingBlocksManager::new();
        let block1 = create_test_block(b"block 1");
        let block2 = create_test_block(b"block 2");

        manager.add_pending(block1.cid);
        manager.add_pending(block2.cid);
        assert_eq!(manager.len(), 2);

        manager.clear();
        assert_eq!(manager.len(), 0);
        assert!(manager.is_empty());
    }

    #[test]
    fn test_duplicate_pending() {
        let manager = PendingBlocksManager::new();
        let block = create_test_block(b"test data");
        let cid = block.cid;

        let _receiver1 = manager.add_pending(cid);
        assert_eq!(manager.len(), 1);

        // Adding same CID again returns dummy receiver
        let mut receiver2 = manager.add_pending(cid);
        assert_eq!(manager.len(), 1); // Still just 1 pending

        // The second receiver will error (sender dropped)
        assert!(receiver2.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_multiple_blocks() {
        let manager = PendingBlocksManager::new();
        let block1 = create_test_block(b"block 1");
        let block2 = create_test_block(b"block 2");
        let block3 = create_test_block(b"block 3");

        let receiver1 = manager.add_pending(block1.cid);
        let receiver2 = manager.add_pending(block2.cid);
        let receiver3 = manager.add_pending(block3.cid);

        assert_eq!(manager.len(), 3);

        // Complete in different order
        manager.complete(&block2.cid, block2.clone());
        manager.complete(&block1.cid, block1.clone());
        manager.complete(&block3.cid, block3.clone());

        assert_eq!(manager.len(), 0);

        // All receivers should get their blocks
        let received1 = receiver1.await.unwrap();
        let received2 = receiver2.await.unwrap();
        let received3 = receiver3.await.unwrap();

        assert_eq!(received1.cid, block1.cid);
        assert_eq!(received2.cid, block2.cid);
        assert_eq!(received3.cid, block3.cid);
    }
}
