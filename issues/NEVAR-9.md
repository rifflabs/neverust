# NEVAR-9: Add Error Handling (thiserror)

**Phase**: 0 | **Status**: Todo | **Priority**: High

## Description
Implement comprehensive error handling using `thiserror` crate. Define error types for P2P, config, IO, and other operations. Ensure all errors are properly propagated and logged.

## Acceptance Criteria
- [ ] Define P2PError enum with thiserror
- [ ] Define ConfigError enum
- [ ] Define RuntimeError enum
- [ ] Implement Display and Error traits
- [ ] Add context to errors
- [ ] Test error propagation
- [ ] Committed

## Relationships
- **Blocked by**: NEVAR-1
- **Relates to**: ALL (error handling is cross-cutting)
- **Start after**: NEVAR-2

## Technical Notes
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArchivistError {
    #[error("P2P error: {0}")]
    P2P(#[from] P2PError),

    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

## Time Estimate
15 minutes
