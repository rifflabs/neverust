//! Configuration management for Neverust
//!
//! Handles CLI argument parsing, config file loading, and defaults.

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Invalid configuration: {0}")]
    Invalid(String),
}

#[derive(Parser, Debug)]
#[command(name = "neverust")]
#[command(about = "Archivist Storage Node in Rust", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Archivist node
    Start(StartCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct StartCommand {
    /// Data directory for node configuration and storage
    #[arg(long, default_value = "./data")]
    pub data_dir: PathBuf,

    /// TCP port for P2P transport
    #[arg(long, default_value_t = 8070)]
    pub listen_port: u16,

    /// UDP port for peer discovery
    #[arg(long, default_value_t = 8090)]
    pub disc_port: u16,

    /// Logging level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub data_dir: PathBuf,
    pub listen_port: u16,
    pub disc_port: u16,
    pub log_level: String,
}

impl Config {
    /// Create config from CLI arguments
    pub fn from_cli() -> Result<Self, ConfigError> {
        let cli = Cli::parse();

        match cli.command {
            Commands::Start(cmd) => Ok(cmd.into()),
        }
    }

    /// Load config from TOML file, merging with CLI overrides
    pub fn load_from_file(path: &PathBuf) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get default configuration
    pub fn default() -> Self {
        Config {
            data_dir: PathBuf::from("./data"),
            listen_port: 8070,
            disc_port: 8090,
            log_level: "info".to_string(),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.data_dir, PathBuf::from("./data"));
        assert_eq!(config.listen_port, 8070);
        assert_eq!(config.disc_port, 8090);
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_config_from_start_command() {
        let cmd = StartCommand {
            data_dir: PathBuf::from("./test-data"),
            listen_port: 9000,
            disc_port: 9001,
            log_level: "debug".to_string(),
        };

        let config: Config = cmd.into();
        assert_eq!(config.data_dir, PathBuf::from("./test-data"));
        assert_eq!(config.listen_port, 9000);
        assert_eq!(config.disc_port, 9001);
        assert_eq!(config.log_level, "debug");
    }
}
