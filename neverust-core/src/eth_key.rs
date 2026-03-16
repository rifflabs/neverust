//! Ethereum key generation and address derivation.
//!
//! Generates secp256k1 private keys compatible with Ethereum and the
//! Archivist marketplace.  Keys are stored as raw 32-byte hex files
//! (same format Archivist uses for `--eth-private-key`).

use std::path::Path;
use thiserror::Error;
use tiny_keccak::{Hasher, Keccak};
use tracing::info;

#[derive(Error, Debug)]
pub enum EthKeyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid key length: expected 32 bytes, got {0}")]
    InvalidLength(usize),

    #[error("Hex decode error: {0}")]
    HexDecode(String),

    #[error("Secp256k1 error: {0}")]
    Secp256k1(String),
}

/// A raw secp256k1 private key with its derived Ethereum address.
#[derive(Clone)]
pub struct EthKey {
    /// Raw 32-byte secret key.
    secret: [u8; 32],
    /// 20-byte Ethereum address (derived from the uncompressed public key).
    address: [u8; 20],
}

impl EthKey {
    /// Generate a fresh random key.
    pub fn generate() -> Result<Self, EthKeyError> {
        let secret: [u8; 32] = rand::random();
        Self::from_secret(secret)
    }

    /// Construct from raw 32-byte secret.
    pub fn from_secret(secret: [u8; 32]) -> Result<Self, EthKeyError> {
        let address = derive_eth_address(&secret)?;
        Ok(Self { secret, address })
    }

    /// Load from a hex-encoded key file (with or without `0x` prefix).
    pub fn load(path: &Path) -> Result<Self, EthKeyError> {
        let raw = std::fs::read_to_string(path)?;
        let hex_str = raw.trim().strip_prefix("0x").unwrap_or(raw.trim());
        let bytes = hex::decode(hex_str)
            .map_err(|e| EthKeyError::HexDecode(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(EthKeyError::InvalidLength(bytes.len()));
        }
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes);
        Self::from_secret(secret)
    }

    /// Save the secret key to a hex file (no `0x` prefix, matching
    /// the Archivist `--eth-private-key` format).
    pub fn save(&self, path: &Path) -> Result<(), EthKeyError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, hex::encode(self.secret))?;
        // Restrict permissions to owner-only on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    /// Ethereum address as a checksummed `0x`-prefixed string.
    pub fn address_string(&self) -> String {
        checksum_address(&self.address)
    }

    /// Raw 32-byte secret.
    pub fn secret_bytes(&self) -> &[u8; 32] {
        &self.secret
    }
}

/// Load an existing key or generate a new one, saving it to `path`.
pub fn load_or_generate(path: &Path) -> Result<EthKey, EthKeyError> {
    if path.exists() {
        let key = EthKey::load(path)?;
        info!("Loaded ETH key from {:?}: {}", path, key.address_string());
        Ok(key)
    } else {
        let key = EthKey::generate()?;
        key.save(path)?;
        info!(
            "Generated new ETH key at {:?}: {}",
            path,
            key.address_string()
        );
        Ok(key)
    }
}

/// Derive a 20-byte Ethereum address from a 32-byte secp256k1 secret key.
fn derive_eth_address(secret: &[u8; 32]) -> Result<[u8; 20], EthKeyError> {
    use libp2p::identity::secp256k1;

    let sk = secp256k1::SecretKey::try_from_bytes(secret.to_vec())
        .map_err(|e| EthKeyError::Secp256k1(e.to_string()))?;
    let kp = secp256k1::Keypair::from(sk);

    // libp2p exposes `to_bytes_uncompressed()` → 65-byte (04 || X || Y).
    let uncompressed = kp.public().to_bytes_uncompressed();

    // Keccak-256 of the 64-byte payload (skip the 0x04 prefix).
    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(&uncompressed[1..]);
    hasher.finalize(&mut hash);

    // Last 20 bytes of the hash.
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..]);
    Ok(addr)
}

/// EIP-55 checksummed address string.
fn checksum_address(addr: &[u8; 20]) -> String {
    let hex_addr = hex::encode(addr);
    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(hex_addr.as_bytes());
    hasher.finalize(&mut hash);

    let mut result = String::with_capacity(42);
    result.push_str("0x");
    for (i, c) in hex_addr.chars().enumerate() {
        let nibble = (hash[i / 2] >> (if i % 2 == 0 { 4 } else { 0 })) & 0xf;
        if nibble >= 8 {
            result.push(c.to_ascii_uppercase());
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.key");

        let key = EthKey::generate().unwrap();
        key.save(&path).unwrap();

        let loaded = EthKey::load(&path).unwrap();
        assert_eq!(key.secret, loaded.secret);
        assert_eq!(key.address, loaded.address);
        assert_eq!(key.address_string(), loaded.address_string());
    }

    #[test]
    fn test_address_format() {
        let key = EthKey::generate().unwrap();
        let addr = key.address_string();
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42);
    }

    #[test]
    fn test_known_key_derivation() {
        // Well-known test vector:
        // private key: 0x4c0883a69102937d6231471b5dbb6204fe512961708279f3e... (Ethereum wiki)
        // For a simpler check, just verify round-trip and format.
        let secret = [1u8; 32]; // Simple key for deterministic test.
        let key = EthKey::from_secret(secret).unwrap();
        let addr = key.address_string();
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42);

        // Verify the address is consistent across calls.
        let key2 = EthKey::from_secret(secret).unwrap();
        assert_eq!(key.address_string(), key2.address_string());
    }

    #[test]
    fn test_well_known_vector() {
        // Private key: 1 (0x0000...0001)
        // Known ETH address for secret key = 1:
        // 0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf
        let mut secret = [0u8; 32];
        secret[31] = 1;
        let key = EthKey::from_secret(secret).unwrap();
        assert_eq!(
            key.address_string().to_lowercase(),
            "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
    }

    #[test]
    fn test_load_hex_with_0x_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefixed.key");
        let key = EthKey::generate().unwrap();
        // Save with 0x prefix.
        std::fs::write(&path, format!("0x{}", hex::encode(key.secret))).unwrap();
        let loaded = EthKey::load(&path).unwrap();
        assert_eq!(key.address_string(), loaded.address_string());
    }

    #[test]
    fn test_load_or_generate_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auto.key");
        assert!(!path.exists());

        let key1 = load_or_generate(&path).unwrap();
        assert!(path.exists());

        // Second call loads the same key.
        let key2 = load_or_generate(&path).unwrap();
        assert_eq!(key1.address_string(), key2.address_string());
    }
}
