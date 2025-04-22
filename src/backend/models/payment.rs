// src/backend/models/payment.rs
// Placeholder for PaymentSession struct and related payment models 

use crate::models::common::{PrincipalId, Timestamp};
use crate::error::{VaultError};
use candid::{CandidType, Deserialize};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use ic_stable_structures::{storable::Bound, Storable};
use std::borrow::Cow;
pub type E8s = u64; // Amount in 10^-8 ICP

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum PayMethod {
    IcpDirect,
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum PayState {
    Issued,    // Session created, waiting for payment
    Pending,   // Payment verification in progress
    Confirmed, // Payment verified on ledger
    Closed,    // Vault created/session finalized
    Expired,   // Session timed out before confirmation
    Error,     // An error occurred during processing
}

impl Default for PayMethod { fn default() -> Self { PayMethod::IcpDirect } }
impl Default for PayState { fn default() -> Self { PayState::Issued } }

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct PaymentSession {
    pub session_id: PrincipalId,
    pub pay_to_account_id: String,   // ICP AccountIdentifier (derived from this canister + subaccount)
    pub pay_to_subaccount: Option<[u8; 32]>, // Store the raw subaccount bytes
    pub amount_e8s: E8s,             // Amount expected in ICP e8s
    pub vault_plan: String,           // e.g., "Standard", "Premium"
    pub method: PayMethod,           // Always IcpDirect for MVP
    pub state: PayState,
    pub initiating_principal: PrincipalId, // Who started the payment process
    pub created_at: Timestamp,          // Nanoseconds since epoch
    pub expires_at: Timestamp,          // Nanoseconds since epoch when the session expires
    pub verified_at: Option<Timestamp>,  // When payment was confirmed
    pub closed_at: Option<Timestamp>,    // When vault was successfully created post-payment
    pub error_message: Option<String>,     // Details if state is Error
    pub ledger_tx_hash: Option<String>,     // Confirmation detail (e.g., "block_12345")
}

impl PaymentSession {
    /// Helper to check if session is expired based on current time.
    pub fn is_expired(&self, current_time: Timestamp) -> bool {
        // Only Issued or Pending sessions can expire
        (self.state == PayState::Issued || self.state == PayState::Pending)
            && current_time > self.expires_at
    }
}

// Implement Storable for stable memory persistence
impl Storable for PaymentSession {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut writer = Vec::new();
        ciborium::into_writer(&self, &mut writer).expect("Failed to serialize PaymentSession");
        Cow::Owned(writer)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        ciborium::from_reader(bytes.as_ref()).expect("Failed to deserialize PaymentSession")
    }

    // Max size estimate based on fields (approximate)
    const BOUND: Bound = Bound::Bounded { max_size: 512, is_fixed_size: false };
}

// --- In-Memory Store for Payment Sessions ---
// Used for MVP; cleared on upgrade. Consider stable storage if persistence needed.
thread_local! {
    static PAYMENT_SESSIONS: RefCell<HashMap<PrincipalId, PaymentSession>> = RefCell::new(HashMap::new());
}

/// Stores a payment session in the in-memory map.
pub fn store_payment_session(session: PaymentSession) {
    PAYMENT_SESSIONS.with(|map| {
        map.borrow_mut().insert(session.session_id.clone(), session);
    });
}

/// Retrieves a mutable reference to a payment session from the in-memory map.
pub fn with_payment_session_mut<F, R>(session_id: &PrincipalId, f: F) -> Result<R, VaultError>
where
    F: FnOnce(&mut PaymentSession) -> Result<R, VaultError>,
{
    PAYMENT_SESSIONS.with(|map| {
        let mut borrowed_map = map.borrow_mut();
        borrowed_map
            .get_mut(session_id)
            .ok_or_else(|| VaultError::PaymentError("Payment session not found".to_string()))
            .and_then(f)
    })
}

/// Retrieves an immutable reference to a payment session from the in-memory map.
pub fn with_payment_session<F, R>(session_id: &PrincipalId, f: F) -> Result<R, VaultError>
where
    F: FnOnce(&PaymentSession) -> Result<R, VaultError>,
{
    PAYMENT_SESSIONS.with(|map| {
        let borrowed_map = map.borrow();
        borrowed_map
            .get(session_id)
            .ok_or_else(|| VaultError::PaymentError("Payment session not found".to_string()))
            .and_then(f)
    })
}
