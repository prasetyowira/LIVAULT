// src/backend/models/payment.rs
// Placeholder for PaymentSession struct and related payment models 

use crate::models::common::{PrincipalId, Timestamp};
use candid::{CandidType, Deserialize, Nat};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;

pub type SessionId = String;
pub type E8s = u64; // Amount in 10^-8 ICP

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum PayMethod {
    IcpDirect,
    ChainFusion, // Deferred implementation
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum PayState {
    Issued,    // Session created, waiting for payment
    Confirmed, // Payment verified on ledger
    Processing, // ChainFusion swap in progress (future)
    Closed,    // Vault created successfully using this payment
    Expired,   // Timeout reached without payment confirmation
    Error,     // An error occurred during processing
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct PaymentSession {
    pub session_id: SessionId,       // ULID
    pub pay_to_principal: PrincipalId, // Temp sub-account for payment
    pub amount_e8s: E8s,
    pub vault_plan: String,        // Plan associated with this payment
    pub method: PayMethod,
    pub state: PayState,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,      // e.g., 30 minutes from creation
    pub verified_at: Option<Timestamp>,
    pub closed_at: Option<Timestamp>,
    pub error_message: Option<String>,
    // Fields for ChainFusion (deferred)
    // pub swap_address: Option<String>,
    // pub token_symbol: Option<String>,
    // pub original_token_amount: Option<String>,
}

impl PaymentSession {
    // Helper to check if session is expired
    pub fn is_expired(&self, current_time: Timestamp) -> bool {
        self.state == PayState::Issued && current_time > self.expires_at
    }
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
    pub created_at_time: Option<ic_cdk::export::candid::Nat>,
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