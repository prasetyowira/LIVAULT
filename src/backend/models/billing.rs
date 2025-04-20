// src/backend/models/billing.rs
use candid::{CandidType, Nat, Principal};
use serde::{Deserialize, Serialize};
use ic_stable_structures::Storable;
use std::borrow::Cow;
use ic_stable_structures::storable::Bound;

// Matches the definition in backend.architecture.md & api.rs
#[derive(CandidType, Deserialize, Serialize, Clone, Debug, Default)]
pub struct BillingEntry {
     pub date: u64, // Timestamp (epoch sec)
     pub vault_id: String, // VaultId
     pub tx_type: String, // e.g., "purchase", "upgrade"
     pub amount_icp_e8s: u64, // Amount paid in ICP equivalent
     pub original_token: Option<String>, // e.g., "ICP", "ETH", "USDT"
     pub original_amount: Option<String>, // Amount in original token (String for precision)
     pub payment_method: String, // e.g., "IcpDirect", "ChainFusion"
     pub ledger_tx_hash: Option<String>, // ICP ledger transaction hash
     pub swap_tx_hash: Option<String>, // Optional hash from the source chain (e.g., ETH tx hash)
     pub related_principal: Option<Principal>, // e.g., payer or vault owner
}

// Implement Storable for use with StableLog or StableBTreeMap
impl Storable for BillingEntry {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(ciborium::into_writer(&self, Vec::new()).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        ciborium::from_reader(bytes.as_ref()).unwrap()
    }

    // Adjust bound based on expected size and usage (Log vs Map)
    // For StableLog, Bound::Unbounded is often fine.
    // For StableBTreeMap, a max size might be needed.
    const BOUND: Bound = Bound::Unbounded; // Assuming use with StableLog initially
} 