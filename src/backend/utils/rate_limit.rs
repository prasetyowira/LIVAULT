// src/backend/utils/rate_limit.rs
use crate::error::VaultError;
use candid::Principal; // Import Nat
use ic_cdk::api::time;
use std::cell::RefCell;
use std::collections::HashMap;

// --- Configuration ---
const RATE_LIMIT_CAPACITY: u32 = 20; // Max tokens in bucket (burst capacity)
const RATE_LIMIT_REFILL_RATE_PER_SEC: f64 = 1.0; // Tokens added per second

struct TokenBucket {
    tokens: f64,
    last_refill_time_ns: u64,
}

impl TokenBucket {
    fn new() -> Self {
        TokenBucket {
            tokens: RATE_LIMIT_CAPACITY as f64,
            last_refill_time_ns: time(),
        }
    }

    fn refill(&mut self) {
        let now_ns = time();
        let elapsed_secs = (now_ns.saturating_sub(self.last_refill_time_ns)) as f64 / 1_000_000_000.0;
        let tokens_to_add = elapsed_secs * RATE_LIMIT_REFILL_RATE_PER_SEC;

        self.tokens = (self.tokens + tokens_to_add).min(RATE_LIMIT_CAPACITY as f64);
        self.last_refill_time_ns = now_ns;
    }

    fn take(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

thread_local! {
    // In-memory map for rate limiting. Cleared on upgrade.
    static PRINCIPAL_BUCKETS: RefCell<HashMap<Principal, TokenBucket>> = RefCell::new(HashMap::new());
}

/// Guard function for rate limiting canister calls.
/// Returns Ok(()) if the call is allowed, Err(VaultError::RateLimitExceeded) otherwise.
pub fn rate_guard() -> Result<(), String> {
    let caller = ic_cdk::caller();

    // Allow anonymous calls for certain endpoints if needed (e.g., metrics?) - Skip for now
    // if caller == Principal::anonymous() {
    //     return Ok(());
    // }

    PRINCIPAL_BUCKETS.with(|buckets_refcell| {
        let mut buckets = buckets_refcell.borrow_mut();
        let bucket = buckets.entry(caller).or_insert_with(TokenBucket::new);

        if bucket.take() {
            Ok(())
        } else {
            Err(VaultError::RateLimitExceeded(format!(
                "Rate limit exceeded for principal {}. Please try again later.",
                caller
            )).to_string()) // Convert VaultError to String for the guard signature
        }
    })
} 