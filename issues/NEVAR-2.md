# NEVAR-2: Set up Cargo Workspace

**Phase**: 0 | **Status**: Todo | **Priority**: Critical

## Description
Create Rust Cargo workspace with binary crate (`neverust`) and library crate (`neverust-core`). Configure dependencies for libp2p, tokio, clap, tracing, and testing frameworks.

## Acceptance Criteria
- [ ] Test: Workspace builds successfully with `cargo build`
- [ ] Create root `Cargo.toml` with workspace configuration
- [ ] Create `neverust` binary crate in `src/`
- [ ] Create `neverust-core` library crate in `neverust-core/`
- [ ] Add dependencies: tokio, libp2p, clap, tracing, thiserror
- [ ] Configure `tests/` directory for integration tests
- [ ] `cargo test` runs (even with no tests yet)
- [ ] Implementation complete and tests pass
- [ ] Committed

## Relationships
- **Blocked by**: NEVAR-1 (issue tracking must be ready)
- **Blocking**: NEVAR-3 (CLI needs workspace), NEVAR-4 (P2P needs workspace)
- **Start before**: NEVAR-3
- **Finish before**: NEVAR-3, NEVAR-4

## Technical Notes

**Workspace Structure**:
```
neverust/
├── Cargo.toml          # Workspace root
├── neverust-core/      # Library crate
│   ├── Cargo.toml
│   └── src/lib.rs
├── src/                # Binary crate
│   └── main.rs
└── tests/
    └── integration_test.rs
```

**Dependencies** (Cargo.toml):
```toml
[workspace]
members = ["neverust-core"]

[package]
name = "neverust"
version = "0.1.0"
edition = "2021"

[dependencies]
neverust-core = { path = "./neverust-core" }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
```

**Library Dependencies** (neverust-core/Cargo.toml):
```toml
[package]
name = "neverust-core"
version = "0.1.0"
edition = "2021"

[dependencies]
libp2p = { version = "0.53", features = ["tcp", "noise", "yamux", "ping", "identify", "kad"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
thiserror = "1.0"
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
```

## Time Estimate
15 minutes
