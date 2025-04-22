// src/backend/utils/rng.rs

use rand_chacha::{{ChaCha8Rng, rand_core::SeedableRng}};
use std::cell::RefCell;
use ic_cdk::api::management_canister::main::raw_rand;
use crate::error::VaultError; // Assuming VaultError exists

thread_local! {
    // Separate RNG specifically for internal cryptographic operations like Shamir,
    // distinct from the one potentially used for getrandom hook.
    static INTERNAL_RNG: RefCell<Option<ChaCha8Rng>> = RefCell::new(None);
}

/// Initializes the thread-local ChaCha8Rng using raw_rand from the IC.
/// Should be called during canister init and post_upgrade.
pub async fn initialize_internal_rng() -> Result<(), VaultError> {
    let raw: Result<(Vec<u8>,), _> = raw_rand().await;
    match raw {
        Ok((bytes,)) => {
            if bytes.len() >= 32 {
                let seed: [u8; 32] = bytes[..32].try_into()
                    .map_err(|_| VaultError::InternalError("Failed to create seed from raw_rand".to_string()))?;
                INTERNAL_RNG.with(|rng| {
                    *rng.borrow_mut() = Some(ChaCha8Rng::from_seed(seed));
                });
                ic_cdk::print("Internal RNG initialized successfully.");
                Ok(())
            } else {
                ic_cdk::print("Error: raw_rand returned insufficient bytes for seed");
                Err(VaultError::InternalError("raw_rand returned insufficient bytes for seed".to_string()))
            }
        }
        Err(e) => {
            ic_cdk::print(format!("Error fetching raw_rand: {:?}", e));
            Err(VaultError::InternalError(format!("Failed to get raw_rand: {:?}", e)))
        }
    }
}

/// Borrows the initialized internal RNG.
/// Panics if the RNG has not been initialized.
pub fn with_internal_rng<F, R>(f: F) -> R
    where F: FnOnce(&mut ChaCha8Rng) -> R
{
    INTERNAL_RNG.with(|rng| {
        let mut borrowed_rng = rng.borrow_mut();
        let rng_instance = borrowed_rng.as_mut()
            .expect("Internal RNG accessed before initialization");
        f(rng_instance)
    })
}

// Example usage (not part of the actual module, just for illustration):
// fn use_the_rng() {
//     let random_byte = with_internal_rng(|rng| {
//         let mut buf = [0u8; 1];
//         rng.fill_bytes(&mut buf);
//         buf[0]
//     });
//     ic_cdk::print(format!("Generated random byte: {}", random_byte));
// } 