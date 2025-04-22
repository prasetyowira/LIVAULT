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
     pub tx_type: String, // e.g., "Vault Creation", "Upgrade"
     pub amount_icp_e8s: u64, // Amount paid in ICP equivalent
     pub payment_method: String, // Always "IcpDirect" for MVP
     pub ledger_tx_hash: Option<String>, // ICP ledger transaction hash
     pub related_principal: Option<Principal>, // e.g., payer or vault owner
}

// Implement Storable for use with StableLog or StableBTreeMap
impl Storable for BillingEntry {
    fn to_bytes(&self) -> Cow<[u8]> {
        // Create a Vec<u8> writer, write into it, and return the owned Vec
        let mut writer = Vec::new();
        ciborium::into_writer(&self, &mut writer).expect("Failed to serialize BillingEntry");
        Cow::Owned(writer)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        ciborium::from_reader(bytes.as_ref()).unwrap()
    }

    // Adjust bound based on expected size and usage (Log vs Map)
    // For StableLog, Bound::Unbounded is often fine.
    // For StableBTreeMap, a max size might be needed.
    const BOUND: Bound = Bound::Unbounded; // Assuming use with StableLog initially
} 