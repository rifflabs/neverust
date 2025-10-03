# NEVAR-7: Add Structured Logging (tracing)

**Phase**: 0 | **Status**: Todo | **Priority**: High

## Description
Implement structured logging using the `tracing` crate with `tracing-subscriber`. Configure log levels, formatting, and ensure all major events (connections, peer discovery, errors) are properly logged.

## Acceptance Criteria
- [ ] Configure tracing subscriber with env filter
- [ ] Log node startup with peer ID
- [ ] Log listening addresses
- [ ] Log connection events (established, closed)
- [ ] Log Ping and Identify events
- [ ] Support --log-level CLI option
- [ ] Committed

## Relationships
- **Blocked by**: NEVAR-1
- **Relates to**: NEVAR-5 (event loop uses logging)
- **Start after**: NEVAR-2

## Technical Notes
```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_logging(level: &str) {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(level))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
```

## Time Estimate
15 minutes
