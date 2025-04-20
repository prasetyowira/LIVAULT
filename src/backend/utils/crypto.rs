// src/backend/utils/crypto.rs
// Placeholder for cryptographic utilities (hashing, encryption helpers)

/// Generates a ULID-like unique string using raw randomness from the IC.
///
/// # Important
/// This function uses `ic_cdk::block_on` and **can only be called from an `#[update]` context**.
/// Calling this from a `#[query]` context will trap.
pub async fn generate_ulid() -> String {
    // Block on the async call to get random bytes.
    // This is only safe within an #[update] call.
    let result = ic_cdk::api::management_canister::main::raw_rand().await;

    // Convert the result (Vec<u8>) to a hex string or another suitable format.
    // Using hex representation for simplicity here. A proper ULID implementation would be more complex.
    match result {
        Ok(bytes) => hex::encode(bytes),
        Err(e) => {
            // Handle the error appropriately, maybe trap or return a default/error indicator.
            // Trapping might be reasonable if randomness is critical.
            ic_cdk::trap(&format!("Failed to get random bytes: {:?}", e));
        }
    }
}

// Other crypto functions... 