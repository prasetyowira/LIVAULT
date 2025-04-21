// src/backend/utils/crypto.rs
// Placeholder for cryptographic utilities (hashing, encryption helpers)

use crate::error::VaultError;
use ic_cdk::api::management_canister::main::raw_rand;
use sha2::{Digest, Sha256};
use hex;
use candid::Principal;

/// Generates random bytes using `raw_rand`.
pub async fn generate_random_bytes(num_bytes: usize) -> Result<Vec<u8>, VaultError> {
    // Note: raw_rand returns 32 bytes. If more are needed, multiple calls might be necessary,
    // but that increases cycle cost and complexity significantly.
    if num_bytes > 32 {
        return Err(VaultError::InternalError(
            "Cannot request more than 32 random bytes from raw_rand in one call".to_string(),
        ));
    }
    let (bytes,) = raw_rand().await.map_err(|(code, msg)| {
        VaultError::InternalError(format!("raw_rand failed: code={}, msg={}", code as u8, msg))
    })?;
    // Return only the requested number of bytes
    Ok(bytes.get(..num_bytes).ok_or_else(|| VaultError::InternalError("Failed to slice random bytes".to_string()))?.to_vec())
}

/// Generates a new, unique Principal based on raw_rand via generate_random_bytes.
/// Ensure this is called from an async context.
pub async fn generate_unique_principal() -> Result<Principal, VaultError> {
    // Generate 29 bytes for a self-authenticating ID
    let rand_bytes = generate_random_bytes(29).await?;

    // Add the self-authenticating suffix (0x02)
    // Use slice concatenation for efficiency
    let mut principal_bytes = Vec::with_capacity(30);
    principal_bytes.extend_from_slice(&rand_bytes);
    principal_bytes.push(0x02); // Suffix for self-authenticating ID

    Ok(Principal::from_slice(&principal_bytes))
}

/// Generates a secure random hex string of a specific byte length.
pub async fn generate_random_hex_string(num_bytes: usize) -> Result<String, VaultError> {
    let bytes = generate_random_bytes(num_bytes).await?;
    Ok(hex::encode(&bytes))
}

/// Calculates the SHA256 hash of byte data and returns it as a hex string.
pub fn calculate_sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

// Other crypto functions... 