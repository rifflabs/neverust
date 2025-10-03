//! Neverust Core
//!
//! Core P2P networking and storage functionality for the Archivist node.

pub mod blockexc;
pub mod config;
pub mod p2p;
pub mod runtime;
pub mod spr;

pub use config::Config;
pub use p2p::{create_swarm, Behaviour, BehaviourEvent, P2PError};
pub use runtime::run_node;
pub use spr::{parse_spr_records, SprError};
