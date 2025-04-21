// src/backend/models/payment.rs
// Placeholder for PaymentSession struct and related payment models 

use crate::models::common::{PrincipalId, Timestamp};
use crate::error::{VaultError};
use candid::{CandidType, Deserialize, Nat};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use ic_stable_structures::{storable::Bound, Storable};
use std::borrow::Cow;
pub type E8s = u64; // Amount in 10^-8 ICP

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum PayMethod {
    IcpDirect,
    ChainFusion, // Deferred implementation
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum PayState {
    Issued,    // Session created, waiting for payment
    Pending,   // Payment detected (e.g., CF swap processing), waiting for confirmation
    Confirmed, // Payment verified on ledger/CF
    Closed,    // Vault created successfully after confirmation
    Expired,   // Session timed out before confirmation
    Error,     // An error occurred during processing
}

impl Default for PayMethod { fn default() -> Self { PayMethod::IcpDirect } }
impl Default for PayState { fn default() -> Self { PayState::Issued } }

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct PaymentSession {
    pub session_id: PrincipalId,
    pub pay_to_account_id: String,   // ICP AccountIdentifier (derived from this canister + subaccount)
    pub amount_e8s: E8s,             // Amount expected in ICP e8s
    pub vault_plan: String,           // e.g., "Standard", "Premium"
    pub method: PayMethod,
    pub state: PayState,
    pub initiating_principal: PrincipalId, // Who started the payment process
    pub created_at: Timestamp,          // Nanoseconds since epoch
    pub expires_at: Timestamp,          // Nanoseconds since epoch when the session expires
    pub verified_at: Option<Timestamp>,  // When payment was confirmed
    pub closed_at: Option<Timestamp>,    // When vault was successfully created post-payment
    pub error_message: Option<String>,     // Details if state is Error
    pub ledger_tx_hash: Option<String>,     // ICP ledger transaction hash if confirmed

    // ChainFusion specific fields
    pub chainfusion_swap_address: Option<String>, // e.g., ETH address user needs to send to
    pub chainfusion_source_token: Option<String>, // e.g., "ETH", "USDT"
    pub chainfusion_source_amount: Option<String>, // Estimated amount in source token (string for precision)
}

impl PaymentSession {
    // Helper to check if session is expired
    pub fn is_expired(&self, current_time: Timestamp) -> bool {
        self.state == PayState::Issued && current_time > self.expires_at
    }
}

// Implement Storable for stable memory persistence
impl Storable for PaymentSession {
    fn to_bytes(&self) -> Cow<[u8]> {
        // Create a Vec<u8> writer, write into it, and return the owned Vec
        let mut writer = Vec::new();
        ciborium::into_writer(&self, &mut writer).expect("Failed to serialize PaymentSession");
        Cow::Owned(writer)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        ciborium::from_reader(bytes.as_ref()).unwrap()
    }

    // Estimate max size: ULID (26) + AccountID (64) + E8s (8) + Plan (30) +
    // Method/State (~10) + Principal (29) + Timestamps (3*8=24) +
    // Error (~100) + Hashes (2*66=132) + CF fields (~100 + 10 + 20 = 130)
    // ~ 553 bytes. Round up generously.
    const BOUND: Bound = Bound::Bounded { max_size: 600, is_fixed_size: false };
}

// --- In-Memory Store for Payment Sessions ---
// Cleared on upgrade. Persistence could be added later if needed.
thread_local! {
    static PAYMENT_SESSIONS: RefCell<HashMap<SessionId, PaymentSession>> = RefCell::new(HashMap::new());
}

/// Stores a payment session in the in-memory map.
pub fn store_payment_session(session: PaymentSession) {
    PAYMENT_SESSIONS.with(|map| {
        map.borrow_mut().insert(session.session_id.clone(), session);
    });
}

/// Retrieves a mutable reference to a payment session.
pub fn with_payment_session_mut<F, R>(session_id: &SessionId, f: F) -> Result<R, VaultError>
where
    F: FnOnce(&mut PaymentSession) -> Result<R, VaultError>,
{
    PAYMENT_SESSIONS.with(|map| {
        let mut borrowed_map = map.borrow_mut();
        let session = borrowed_map
            .get_mut(session_id)
            .ok_or_else(|| VaultError::PaymentError("Payment session not found".to_string()))?;
        f(session)
    })
}

/// Retrieves an immutable reference to a payment session.
pub fn with_payment_session<F, R>(session_id: &SessionId, f: F) -> Result<R, VaultError>
where
    F: FnOnce(&PaymentSession) -> Result<R, VaultError>,
{
    PAYMENT_SESSIONS.with(|map| {
        let borrowed_map = map.borrow();
        let session = borrowed_map
            .get(session_id)
            .ok_or_else(|| VaultError::PaymentError("Payment session not found".to_string()))?;
        f(session)
    })
}

// --- Ledger Interaction Types (from icp_ledger crate example) ---
// These are placeholders; actual types depend on the ledger crate used.

#[derive(CandidType, Deserialize, Debug)] pub struct AccountIdentifier(pub String);
#[derive(CandidType, Deserialize, Debug)] pub struct Tokens { pub e8s: u64, }
#[derive(CandidType, Deserialize, Debug)] pub struct Memo(pub u64);
#[derive(CandidType, Deserialize, Debug)] pub struct Subaccount(pub [u8; 32]);

#[derive(CandidType, Deserialize, Debug)]
pub struct TransferArgs {
    pub memo: Memo,
    pub amount: Tokens,
    pub fee: Tokens,
    pub from_subaccount: Option<Subaccount>,
    pub to: AccountIdentifier,
    pub created_at_time: Option<Nat>,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum TransferError {
    BadFee { expected_fee: Tokens },
    BadBurn { min_burn_amount: Tokens },
    InsufficientFunds { balance: Tokens },
    TooOld,
    CreatedInFuture { ledger_time: u64 },
    Duplicate { duplicate_of: Nat },
    TemporarilyUnavailable,
    GenericError{ error_code: Nat, message: String },
}

#[derive(CandidType, Deserialize, Debug)]
pub struct BlockIndex(pub u64);

// Placeholder for querying blocks
#[derive(CandidType, Deserialize, Debug)]
pub struct GetBlocksArgs { pub start: u64, pub length: usize }

// Placeholder structure for a transaction retrieved from the ledger
// Adapt this based on the actual ledger query results
#[derive(CandidType, Deserialize, Debug)]
pub struct LedgerTransaction {
    // Define fields based on ledger get_blocks or query_tx response
    pub memo: Memo,
    pub amount: Tokens,
    pub receiver: AccountIdentifier, // or Principal?
    pub sender: AccountIdentifier,
    pub block_index: BlockIndex,
    pub timestamp: u64, // or specific timestamp type from ledger
    // ... other relevant fields
} 