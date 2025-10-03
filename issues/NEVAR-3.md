# NEVAR-3: Implement CLI Framework (clap)

**Phase**: 0 | **Status**: Todo | **Priority**: Critical

## Description
Build command-line interface using clap with derive macros. Implement `start` subcommand with options for `--data-dir`, `--listen-port`, `--disc-port`, and `--log-level`. Parse CLI args into Config struct.

## Acceptance Criteria
- [ ] Test: CLI parses `start` command successfully
- [ ] Test: `--data-dir` option sets correct path
- [ ] Test: `--listen-port` option sets correct port
- [ ] Test: Default values apply when options not provided
- [ ] Test: Invalid arguments produce helpful error messages
- [ ] Implement: clap CLI with derive macros
- [ ] Implement: `StartCommand` struct
- [ ] Implement: Config struct with defaults
- [ ] Implementation complete and all tests pass
- [ ] Committed atomically

## Relationships
- **Blocked by**: NEVAR-2 (workspace must exist)
- **Blocking**: NEVAR-5 (event loop uses config)
- **Relates to**: NEVAR-8 (config file loading extends this)
- **Start after**: NEVAR-2

## Technical Notes

**Test First** (TDD):
```rust
// src/cli.rs tests
#[test]
fn test_cli_parses_start_command() {
    let args = vec!["neverust", "start", "--data-dir", "./test-data", "--listen-port", "8070"];
    let config = parse_cli_args(args).unwrap();
    assert_eq!(config.data_dir, PathBuf::from("./test-data"));
    assert_eq!(config.listen_port, 8070);
}

#[test]
fn test_cli_uses_defaults() {
    let args = vec!["neverust", "start"];
    let config = parse_cli_args(args).unwrap();
    assert_eq!(config.data_dir, PathBuf::from("./data"));
    assert_eq!(config.listen_port, 8070);
    assert_eq!(config.disc_port, 8090);
}
```

**Implementation**:
```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "neverust")]
#[command(about = "Archivist Storage Node in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Archivist node
    Start(StartCommand),
}

#[derive(Parser, Debug)]
struct StartCommand {
    /// Data directory for node configuration and storage
    #[arg(long, default_value = "./data")]
    data_dir: PathBuf,

    /// TCP port for P2P transport
    #[arg(long, default_value_t = 8070)]
    listen_port: u16,

    /// UDP port for peer discovery
    #[arg(long, default_value_t = 8090)]
    disc_port: u16,

    /// Logging level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}
```

**Config Struct**:
```rust
#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub listen_port: u16,
    pub disc_port: u16,
    pub log_level: String,
}

impl From<StartCommand> for Config {
    fn from(cmd: StartCommand) -> Self {
        Config {
            data_dir: cmd.data_dir,
            listen_port: cmd.listen_port,
            disc_port: cmd.disc_port,
            log_level: cmd.log_level,
        }
    }
}
```

**Crate Versions**:
- clap = "4" with derive feature

## Time Estimate
20 minutes (TDD cycle)
