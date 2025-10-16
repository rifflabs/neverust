//! Prometheus metrics for benchmarking and monitoring
//!
//! Thread-safe metrics collection using atomic types

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Global metrics collector for Neverust node
#[derive(Clone)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    // Peer connection metrics
    peer_connections: AtomicUsize,
    total_peers_seen: AtomicU64,

    // Block transfer metrics
    blocks_sent: AtomicU64,
    blocks_received: AtomicU64,

    // Byte transfer metrics
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,

    // Cache metrics (for future multi-tier cache)
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,

    // Block exchange latency (simple moving average in milliseconds)
    total_exchange_time_ms: AtomicU64,
    total_exchanges: AtomicU64,

    // Discovery-assisted retrieval metrics
    discovery_queries: AtomicU64,
    discovery_successes: AtomicU64,
    discovery_failures: AtomicU64,
    blocks_from_discovery: AtomicU64,

    // Node start time for uptime calculation
    start_time: SystemTime,
}

impl Metrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                peer_connections: AtomicUsize::new(0),
                total_peers_seen: AtomicU64::new(0),
                blocks_sent: AtomicU64::new(0),
                blocks_received: AtomicU64::new(0),
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
                cache_hits: AtomicU64::new(0),
                cache_misses: AtomicU64::new(0),
                total_exchange_time_ms: AtomicU64::new(0),
                total_exchanges: AtomicU64::new(0),
                discovery_queries: AtomicU64::new(0),
                discovery_successes: AtomicU64::new(0),
                discovery_failures: AtomicU64::new(0),
                blocks_from_discovery: AtomicU64::new(0),
                start_time: SystemTime::now(),
            }),
        }
    }

    // Peer connection metrics

    pub fn peer_connected(&self) {
        self.inner.peer_connections.fetch_add(1, Ordering::Relaxed);
        self.inner.total_peers_seen.fetch_add(1, Ordering::Relaxed);
    }

    pub fn peer_disconnected(&self) {
        self.inner.peer_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn peer_connections(&self) -> usize {
        self.inner.peer_connections.load(Ordering::Relaxed)
    }

    pub fn total_peers_seen(&self) -> u64 {
        self.inner.total_peers_seen.load(Ordering::Relaxed)
    }

    // Block transfer metrics

    pub fn block_sent(&self, size: usize) {
        self.inner.blocks_sent.fetch_add(1, Ordering::Relaxed);
        self.inner
            .bytes_sent
            .fetch_add(size as u64, Ordering::Relaxed);
    }

    pub fn block_received(&self, size: usize) {
        self.inner.blocks_received.fetch_add(1, Ordering::Relaxed);
        self.inner
            .bytes_received
            .fetch_add(size as u64, Ordering::Relaxed);
    }

    pub fn blocks_sent(&self) -> u64 {
        self.inner.blocks_sent.load(Ordering::Relaxed)
    }

    pub fn blocks_received(&self) -> u64 {
        self.inner.blocks_received.load(Ordering::Relaxed)
    }

    pub fn bytes_sent(&self) -> u64 {
        self.inner.bytes_sent.load(Ordering::Relaxed)
    }

    pub fn bytes_received(&self) -> u64 {
        self.inner.bytes_received.load(Ordering::Relaxed)
    }

    // Cache metrics

    pub fn cache_hit(&self) {
        self.inner.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn cache_miss(&self) {
        self.inner.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn cache_hits(&self) -> u64 {
        self.inner.cache_hits.load(Ordering::Relaxed)
    }

    pub fn cache_misses(&self) -> u64 {
        self.inner.cache_misses.load(Ordering::Relaxed)
    }

    // Block exchange latency tracking

    pub fn record_exchange_time(&self, duration_ms: u64) {
        self.inner
            .total_exchange_time_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        self.inner.total_exchanges.fetch_add(1, Ordering::Relaxed);
    }

    pub fn avg_exchange_time_ms(&self) -> f64 {
        let total = self.inner.total_exchange_time_ms.load(Ordering::Relaxed);
        let count = self.inner.total_exchanges.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            total as f64 / count as f64
        }
    }

    // Discovery metrics

    pub fn discovery_query(&self) {
        self.inner.discovery_queries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn discovery_success(&self) {
        self.inner
            .discovery_successes
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn discovery_failure(&self) {
        self.inner
            .discovery_failures
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn block_from_discovery(&self) {
        self.inner
            .blocks_from_discovery
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn discovery_queries(&self) -> u64 {
        self.inner.discovery_queries.load(Ordering::Relaxed)
    }

    pub fn discovery_successes(&self) -> u64 {
        self.inner.discovery_successes.load(Ordering::Relaxed)
    }

    pub fn discovery_failures(&self) -> u64 {
        self.inner.discovery_failures.load(Ordering::Relaxed)
    }

    pub fn blocks_from_discovery(&self) -> u64 {
        self.inner.blocks_from_discovery.load(Ordering::Relaxed)
    }

    pub fn discovery_success_rate(&self) -> f64 {
        let queries = self.discovery_queries();
        if queries == 0 {
            0.0
        } else {
            (self.discovery_successes() as f64 / queries as f64) * 100.0
        }
    }

    // Uptime

    pub fn uptime_seconds(&self) -> u64 {
        self.inner
            .start_time
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Generate Prometheus-formatted metrics text
    pub fn to_prometheus(&self, block_count: usize, total_bytes: usize) -> String {
        format!(
            "# HELP neverust_block_count Total number of blocks stored\n\
             # TYPE neverust_block_count gauge\n\
             neverust_block_count {}\n\
             \n\
             # HELP neverust_block_bytes Total bytes of block data stored\n\
             # TYPE neverust_block_bytes gauge\n\
             neverust_block_bytes {}\n\
             \n\
             # HELP neverust_uptime_seconds Time since node started in seconds\n\
             # TYPE neverust_uptime_seconds counter\n\
             neverust_uptime_seconds {}\n\
             \n\
             # HELP neverust_peer_connections Current number of active peer connections\n\
             # TYPE neverust_peer_connections gauge\n\
             neverust_peer_connections {}\n\
             \n\
             # HELP neverust_total_peers_seen Total number of unique peers seen since start\n\
             # TYPE neverust_total_peers_seen counter\n\
             neverust_total_peers_seen {}\n\
             \n\
             # HELP neverust_blocks_sent_total Total number of blocks sent to peers\n\
             # TYPE neverust_blocks_sent_total counter\n\
             neverust_blocks_sent_total {}\n\
             \n\
             # HELP neverust_blocks_received_total Total number of blocks received from peers\n\
             # TYPE neverust_blocks_received_total counter\n\
             neverust_blocks_received_total {}\n\
             \n\
             # HELP neverust_bytes_sent_total Total bytes sent to peers\n\
             # TYPE neverust_bytes_sent_total counter\n\
             neverust_bytes_sent_total {}\n\
             \n\
             # HELP neverust_bytes_received_total Total bytes received from peers\n\
             # TYPE neverust_bytes_received_total counter\n\
             neverust_bytes_received_total {}\n\
             \n\
             # HELP neverust_cache_hits_total Total number of cache hits\n\
             # TYPE neverust_cache_hits_total counter\n\
             neverust_cache_hits_total {}\n\
             \n\
             # HELP neverust_cache_misses_total Total number of cache misses\n\
             # TYPE neverust_cache_misses_total counter\n\
             neverust_cache_misses_total {}\n\
             \n\
             # HELP neverust_avg_exchange_time_ms Average block exchange time in milliseconds\n\
             # TYPE neverust_avg_exchange_time_ms gauge\n\
             neverust_avg_exchange_time_ms {:.2}\n\
             \n\
             # HELP neverust_discovery_queries_total Total number of discovery queries initiated\n\
             # TYPE neverust_discovery_queries_total counter\n\
             neverust_discovery_queries_total {}\n\
             \n\
             # HELP neverust_discovery_successes_total Total number of successful discovery queries\n\
             # TYPE neverust_discovery_successes_total counter\n\
             neverust_discovery_successes_total {}\n\
             \n\
             # HELP neverust_discovery_failures_total Total number of failed discovery queries\n\
             # TYPE neverust_discovery_failures_total counter\n\
             neverust_discovery_failures_total {}\n\
             \n\
             # HELP neverust_blocks_from_discovery_total Total blocks retrieved via discovery\n\
             # TYPE neverust_blocks_from_discovery_total counter\n\
             neverust_blocks_from_discovery_total {}\n\
             \n\
             # HELP neverust_discovery_success_rate Discovery query success rate (percentage)\n\
             # TYPE neverust_discovery_success_rate gauge\n\
             neverust_discovery_success_rate {:.2}\n",
            block_count,
            total_bytes,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - self.uptime_seconds(),
            self.peer_connections(),
            self.total_peers_seen(),
            self.blocks_sent(),
            self.blocks_received(),
            self.bytes_sent(),
            self.bytes_received(),
            self.cache_hits(),
            self.cache_misses(),
            self.avg_exchange_time_ms(),
            self.discovery_queries(),
            self.discovery_successes(),
            self.discovery_failures(),
            self.blocks_from_discovery(),
            self.discovery_success_rate(),
        )
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_connections() {
        let metrics = Metrics::new();
        assert_eq!(metrics.peer_connections(), 0);

        metrics.peer_connected();
        assert_eq!(metrics.peer_connections(), 1);
        assert_eq!(metrics.total_peers_seen(), 1);

        metrics.peer_connected();
        assert_eq!(metrics.peer_connections(), 2);
        assert_eq!(metrics.total_peers_seen(), 2);

        metrics.peer_disconnected();
        assert_eq!(metrics.peer_connections(), 1);
        assert_eq!(metrics.total_peers_seen(), 2); // Doesn't decrease
    }

    #[test]
    fn test_block_transfers() {
        let metrics = Metrics::new();

        metrics.block_sent(100);
        assert_eq!(metrics.blocks_sent(), 1);
        assert_eq!(metrics.bytes_sent(), 100);

        metrics.block_received(200);
        assert_eq!(metrics.blocks_received(), 1);
        assert_eq!(metrics.bytes_received(), 200);

        metrics.block_sent(50);
        assert_eq!(metrics.blocks_sent(), 2);
        assert_eq!(metrics.bytes_sent(), 150);
    }

    #[test]
    fn test_cache_metrics() {
        let metrics = Metrics::new();

        metrics.cache_hit();
        metrics.cache_hit();
        metrics.cache_miss();

        assert_eq!(metrics.cache_hits(), 2);
        assert_eq!(metrics.cache_misses(), 1);
    }

    #[test]
    fn test_exchange_time() {
        let metrics = Metrics::new();

        metrics.record_exchange_time(100);
        metrics.record_exchange_time(200);

        assert_eq!(metrics.avg_exchange_time_ms(), 150.0);
    }

    #[test]
    fn test_prometheus_output() {
        let metrics = Metrics::new();
        metrics.peer_connected();
        metrics.block_sent(100);

        let output = metrics.to_prometheus(42, 1024);

        assert!(output.contains("neverust_block_count 42"));
        assert!(output.contains("neverust_block_bytes 1024"));
        assert!(output.contains("neverust_peer_connections 1"));
        assert!(output.contains("neverust_blocks_sent_total 1"));
    }
}
