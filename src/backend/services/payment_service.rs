"""// src/backend/services/payment_service.rs
use crate::{
    error::VaultError,
    models::{
        common::*, // Common types like Timestamp, PrincipalId
        payment::*, // Payment models like PaymentSession, PayMethod, PayState, E8s, SessionId
        vault_service, // To potentially trigger vault status change
    },
    storage::payment::{store_payment_session, with_payment_session, with_payment_session_mut}, // Use storage module helpers
    utils::crypto::generate_ulid, // For SessionId
};
use candid::Principal;
use ic_cdk::api::{self, time};
use std::time::Duration;
use ic_ledger_types::{AccountIdentifier, Subaccount, AccountBalanceArgs, TOKEN_SUBDIVIDABLE_BY, DEFAULT_SUBACCOUNT, transfer, TransferArgs, Memo, Tokens};

// Constants
const PAYMENT_SESSION_TIMEOUT_SECONDS: u64 = 30 * 60; // 30 minutes
const ICP_LEDGER_CANISTER_ID_STR: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai"; // Mainnet ICP Ledger

// --- Payment Initialization Struct (from API) ---
#[derive(Clone, Debug, candid::CandidType, serde::Deserialize)]
pub struct PaymentInitRequest {
    pub vault_plan: String,
    pub amount_e8s: E8s,
    pub method: PayMethod, // Initially only IcpDirect
}

// --- Helper: Derive Subaccount from Session ID ---
fn derive_subaccount_from_session(session_id: &SessionId) -> Subaccount {
    // Use the first 32 bytes of the ULID string as entropy for the subaccount
    // Ensure the input is always 32 bytes
    let mut entropy = [0u8; 32];
    let bytes = session_id.as_bytes();
    let len = bytes.len().min(32);
    entropy[..len].copy_from_slice(&bytes[..len]);
    Subaccount(entropy)
}

// --- Service Functions ---

/// Initializes a new payment session.
/// Generates a unique session ID and a temporary principal/subaccount for payment.
///
/// # Arguments
/// * `req` - Payment initialization request details.
/// * `caller` - The principal initiating the payment.
///
/// # Returns
/// * `Result<PaymentSession, VaultError>` - The created payment session details.
pub fn initialize_payment_session(
    req: PaymentInitRequest,
    caller: PrincipalId,
) -> Result<PaymentSession, VaultError> {
    if req.method != PayMethod::IcpDirect {
        return Err(VaultError::PaymentError(
            "Only IcpDirect payment method is supported in this version.".to_string(),
        ));
    }

    let session_id = generate_ulid();
    let current_time = time();
    let expires_at = current_time + Duration::from_secs(PAYMENT_SESSION_TIMEOUT_SECONDS).as_nanos() as u64;

    // Generate a unique subaccount derived from the session ID
    let subaccount = derive_subaccount_from_session(&session_id);
    let vault_canister_principal = api::id(); // This canister's principal
    let pay_to_account = AccountIdentifier::new(&vault_canister_principal, &subaccount);

    let session = PaymentSession {
        session_id: session_id.clone(),
        pay_to_account_id: pay_to_account.to_string(), // Store the AccountIdentifier string
        amount_e8s: req.amount_e8s,
        vault_plan: req.vault_plan,
        method: req.method,
        state: PayState::Issued,
        initiating_principal: caller,
        created_at: current_time,
        expires_at,
        verified_at: None,
        closed_at: None,
        error_message: None,
        ledger_tx_hash: None,
    };

    // Store the session
    store_payment_session(session.clone())?;

    ic_cdk::print(format!(
        "📝 INFO: Payment session {} created for plan {} ({} e8s) by {}. Expires at {}. Pay to Account: {}",
        session_id, session.vault_plan, session.amount_e8s, caller, session.expires_at, session.pay_to_account_id
    ));

    Ok(session)
}

/// Verifies if a payment matching the session details has been confirmed on the ICP Ledger.
///
/// # Arguments
/// * `session_id` - The ID of the payment session to verify.
/// * `vault_id` - The ID of the vault this payment is for (passed after creation attempt).
///
/// # Returns
/// * `Result<String, VaultError>` - Confirmation message (e.g., "Payment Confirmed") or an error.
pub async fn verify_icp_payment(
    session_id: &SessionId,
    vault_id: &VaultId, // Used to update vault status upon confirmation
) -> Result<String, VaultError> {
    let current_time = time();
    let session = with_payment_session(session_id, |s| Ok(s.clone()))?; // Clone to avoid borrow issues

    // 1. Check Session State and Expiry
    match session.state {
        PayState::Confirmed | PayState::Closed => {
            return Ok(format!("Payment already confirmed (Tx: {}).", session.ledger_tx_hash.unwrap_or_default()));
        }
        PayState::Expired => {
            return Err(VaultError::PaymentError("Payment session has expired.".to_string()));
        }
        PayState::Error => {
            return Err(VaultError::PaymentError(format!(
                "Payment session is in error state: {}",
                session.error_message.unwrap_or_default()
            )));
        }
        PayState::Issued => {
            // Check expiry again
            if current_time > session.expires_at {
                // Transition state to Expired
                with_payment_session_mut(session_id, |s| {
                    s.state = PayState::Expired;
                    s.error_message = Some("Session expired before verification.".to_string());
                    Ok(())
                })?;
                return Err(VaultError::PaymentError("Payment session has expired.".to_string()));
            }
            // Proceed to ledger check
        }
        _ => { // Should not happen based on above checks
             return Err(VaultError::InternalError(format!(
                "Verification encountered unexpected session state: {:?}",
                session.state
            )));
        }
    }

    // 2. Query the ICP Ledger Canister
    ic_cdk::print(format!(
        "🔍 DEBUG: Attempting to verify payment for session {} (Amount: {}, To Account: {})",
        session_id, session.amount_e8s, session.pay_to_account_id
    ));

    let ledger_canister_id = Principal::from_text(ICP_LEDGER_CANISTER_ID_STR).expect("Invalid Ledger ID");
    let target_account = AccountIdentifier::from_hex(&session.pay_to_account_id)
        .map_err(|_| VaultError::InternalError("Invalid account identifier stored in session".to_string()))?;

    // --- PLACEHOLDER: Replace with actual ledger query --- 
    // The most robust way is often to check the balance or query recent transactions
    // for the specific derived account identifier.
    // For this example, we simulate success.
    let payment_found_and_valid = true;
    let tx_hash_placeholder = format!("simulated_tx_hash_{}", session_id);
    // --- END PLACEHOLDER --- 

    /*
    // Example: Check account balance (less reliable as proof of specific payment)
    let balance_args = AccountBalanceArgs { account: target_account };
    let balance_result: Result<(Tokens,), _> = api::call::call(ledger_canister_id, "account_balance", (balance_args,)).await;

    match balance_result {
        Ok((balance,)) => {
            if balance.e8s() >= session.amount_e8s {
                payment_found_and_valid = true;
                 ic_cdk::print(format!(
                    "✅ INFO: Account {} balance {} e8s sufficient for payment {} e8s.",
                    session.pay_to_account_id, balance.e8s(), session.amount_e8s
                 ));
                 // NOTE: This doesn't guarantee THIS payment was the one that increased balance.
                 // Needs refinement for production (e.g., checking tx history or using unique memos).
            } else {
                 payment_found_and_valid = false;
                 ic_cdk::print(format!(
                    "⏳ INFO: Account {} balance {} e8s insufficient for payment {} e8s.",
                    session.pay_to_account_id, balance.e8s(), session.amount_e8s
                 ));
            }
        }
        Err((code, msg)) => {
            ic_cdk::eprintln!("🔥 ERROR: Ledger balance query failed for {}: ({:?}) {}", session.pay_to_account_id, code, msg);
            return Err(VaultError::PaymentError("Ledger interaction failed during balance check.".to_string()));
        }
    }
    */

    if payment_found_and_valid {
        ic_cdk::print(format!(
            "✅ INFO: Payment verified for session {} (Tx: {})",
            session_id, tx_hash_placeholder
        ));

        // 3. Update Session State
        with_payment_session_mut(session_id, |s| {
            s.state = PayState::Confirmed;
            s.verified_at = Some(current_time);
            s.error_message = None;
            s.ledger_tx_hash = Some(tx_hash_placeholder.clone()); // Store simulated hash
            Ok(())
        })?;

        // 4. Update Vault Status (Post-Confirmation Action)
        // Transition vault from Draft to NeedSetup (Caller is System/Self here)
        match vault_service::set_vault_status(vault_id, VaultStatus::NeedSetup, Some(api::id())) {
            Ok(_) => {
                 ic_cdk::print(format!(
                    "📝 INFO: Vault {} status updated to NeedSetup after payment confirmation.",
                    vault_id
                 ));
                 // Successfully verified AND updated vault status, now close the session
                 close_payment_session(session_id)?;
                 Ok(format!("Payment Confirmed (Tx: {}). Vault status updated.", tx_hash_placeholder))
            }
            Err(e) => {
                 ic_cdk::eprintln!(
                    "🔥 ERROR: Failed to update vault {} status after payment confirmation: {:?}",
                    vault_id, e
                 );
                 // Keep session as Confirmed but don't close it, flag the error
                 with_payment_session_mut(session_id, |s| {
                    s.state = PayState::Error; // Or a specific state like ConfirmedNeedsRetry?
                    s.error_message = Some(format!("Failed to update vault status: {:?}", e));
                    Ok(())
                 })?;
                 Err(VaultError::InternalError(format!(
                    "Payment confirmed but failed to update vault status: {:?}",
                    e
                 )))
            }
        }
    } else {
        ic_cdk::print(format!("⏳ INFO: Payment not yet detected for session {}", session_id));
        // Return error indicating payment not found yet
        Err(VaultError::PaymentNotFound)
    }
}

/// Closes a payment session, typically after the associated action (e.g., vault creation) is complete.
///
/// # Arguments
/// * `session_id` - The ID of the payment session to close.
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error.
pub fn close_payment_session(session_id: &SessionId) -> Result<(), VaultError> {
     ic_cdk::print(format!("🔒 INFO: Closing payment session {}.", session_id));
    with_payment_session_mut(session_id, |session| {
        match session.state {
            PayState::Confirmed => {
                session.state = PayState::Closed;
                session.closed_at = Some(time());
                ic_cdk::print(format!("✅ INFO: Payment session {} successfully closed.", session_id));
                Ok(())
            },
            PayState::Closed => {
                 ic_cdk::print(format!("⚠️ WARN: Payment session {} already closed.", session_id));
                 Ok(()) // Idempotent
            },
            _ => {
                ic_cdk::eprintln!("🔥 ERROR: Cannot close payment session {} in state {:?}.", session_id, session.state);
                Err(VaultError::PaymentError(format!(
                    "Cannot close payment session in state {:?}",
                    session.state
                )))
            }
        }
    })
}

// TODO: Add function to handle ChainFusion payments (initiate swap, verify swap completion)
// TODO: Add function to get payment session status

"" 