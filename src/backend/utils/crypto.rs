// src/backend/utils/crypto.rs
// Placeholder for cryptographic utilities (hashing, encryption helpers)

use crate::error::VaultError;
use ic_cdk::api::management_canister::main::raw_rand;
use sha2::{Digest, Sha256};
use hex;
use ulid::Ulid;

/// Generates a unique ULID (Universally Unique Lexicographically Sortable Identifier).
pub async fn generate_ulid() -> String {
    // Use raw_rand for entropy if needed, or a simpler time-based approach if sufficient
    // For simplicity here, using time + a bit of randomness
    let time_ms = ic_cdk::api::time() / 1_000_000;
    let (rand_bytes,) = raw_rand().await.unwrap_or_else(|_| (vec![0u8; 16],)); // Ensure 16 bytes for Ulid
    // Use Ulid::from_parts correctly
    Ulid::from_parts(time_ms, u128::from_le_bytes(rand_bytes[..16].try_into().unwrap_or([0; 16]))).to_string()
}

/// Generates random bytes using `raw_rand`.
pub async fn generate_random_bytes(num_bytes: usize) -> Result<Vec<u8>, VaultError> {
    // Note: raw_rand returns 32 bytes. If more are needed, multiple calls might be necessary,
    // but that increases cycle cost and complexity significantly.
    // For typical use cases like IDs or nonces, 32 bytes should be sufficient.
    if num_bytes > 32 {
        return Err(VaultError::InternalError("Cannot request more than 32 random bytes from raw_rand in one call".to_string()));
    }
    let (bytes,) = raw_rand().await.map_err(|(code, msg)| {
        VaultError::InternalError(format!("raw_rand failed: code={}, msg={}", code as u8, msg))
    })?;
    Ok(bytes[..num_bytes].to_vec())
}

/// Generates a secure random hex string of a specific byte length.
pub async fn generate_random_hex_string(num_bytes: usize) -> Result<String, VaultError> {
    let bytes = generate_random_bytes(num_bytes).await?;
    Ok(hex::encode(&bytes)) // Pass bytes as a slice
}

/// Calculates the SHA256 hash of byte data and returns it as a hex string.
pub fn calculate_sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

// Other crypto functions... 