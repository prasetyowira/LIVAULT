// src/backend/services/payment_service.rs
use crate::{
    error::VaultError,
    models::{
        common::*, // Common types like Timestamp, PrincipalId
        payment::*, // Payment models like PaymentSession, PayMethod, PayState, E8s, SessionId
        vault_service, // To potentially trigger vault status change
    },
    storage::payment::{store_payment_session, with_payment_session, with_payment_session_mut}, // Use storage module helpers
    utils::crypto::generate_ulid, // For SessionId
    adapter::chainfusion_adapter::{initialize_chainfusion_swap, check_chainfusion_swap_status, ChainFusionInitRequest, ChainFusionSwapStatus},
    services::vault_service, // Fix vault_service import
};
use candid::Principal;
use ic_cdk::api::{self, time};
use std::time::Duration;
use ic_ledger_types::{AccountIdentifier, Subaccount, AccountBalanceArgs, DEFAULT_SUBACCOUNT, transfer, TransferArgs, Memo, Tokens};
use crate::utils::account_identifier::AccountIdentifier;
use crate::adapter::chainfusion_adapter;
use crate::adapter::ledger_adapter; // Assuming ledger adapter exists
use ic_cdk::api::caller as ic_caller;
use crate::models::billing::BillingEntry;

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
pub async fn initialize_payment_session(
    req: PaymentInitRequest,
    caller: PrincipalId,
) -> Result<PaymentSession, VaultError> {
    let session_id = generate_ulid().await;
    let current_time = time();
    let expires_at = current_time + Duration::from_secs(PAYMENT_SESSION_TIMEOUT_SECONDS).as_nanos() as u64;

    let vault_canister_principal = api::id();
    let subaccount = derive_subaccount_from_session(&session_id);
    let pay_to_account = AccountIdentifier::new(&vault_canister_principal, &subaccount);

    let mut session = PaymentSession {
        session_id: session_id.clone(),
        pay_to_account_id: pay_to_account.to_string(),
        amount_e8s: req.amount_e8s,
        vault_plan: req.vault_plan,
        method: req.method.clone(), // Clone method
        state: PayState::Issued,
        initiating_principal: caller,
        created_at: current_time,
        expires_at,
        verified_at: None,
        closed_at: None,
        error_message: None,
        ledger_tx_hash: None,
        // Initialize ChainFusion specific fields
        chainfusion_swap_address: None,
        chainfusion_source_token: None,
        chainfusion_source_amount: None,
    };

    // Handle ChainFusion initialization if selected
    if req.method == PayMethod::ChainFusion {
        ic_cdk::print(format!(
            "🔗 INFO: Requesting ChainFusion swap details for session {}", session_id
        ));
        let cf_req = ChainFusionInitRequest {
            session_id: session_id.clone(),
            target_principal: pay_to_account.to_string(),
            target_amount_icp_e8s: req.amount_e8s,
        };

        // Call the ChainFusion adapter (async)
        match initialize_chainfusion_swap(cf_req).await {
            Ok(cf_resp) => {
                session.chainfusion_swap_address = Some(cf_resp.swap_address);
                session.chainfusion_source_token = Some(cf_resp.source_token_symbol);
                session.chainfusion_source_amount = Some(cf_resp.estimated_source_amount);
                // Potentially adjust session.expires_at based on cf_resp.expires_at?
                ic_cdk::print(format!(
                    "🔗 INFO: ChainFusion swap initiated. User to send {} to {}",
                    session.chainfusion_source_token.as_deref().unwrap_or("?"),
                    session.chainfusion_swap_address.as_deref().unwrap_or("?")
                ));
            }
            Err(e) => {
                ic_cdk::eprintln!(
                    "🔥 ERROR: Failed to initialize ChainFusion swap for session {}: {:?}",
                    session_id, e
                );
                // Store the error and set state
                session.state = PayState::Error;
                session.error_message = Some(format!("ChainFusion init failed: {:?}", e));
                store_payment_session(session.clone());
                return Err(VaultError::PaymentError(
                    "Failed to initialize ChainFusion swap.".to_string(),
                ));
            }
        }
    }

    // Store the session
    store_payment_session(session.clone());

    ic_cdk::print(format!(
        "📝 INFO: Payment session {} created for plan {} ({} e8s) by {}. Method: {:?}. Expires at {}. Pay to Account: {}",
        session_id, session.vault_plan, session.amount_e8s, caller, session.method, session.expires_at, session.pay_to_account_id
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
pub async fn verify_payment(
    session_id: &SessionId,
    vault_id: &VaultId, // Used to update vault status upon confirmation
) -> Result<String, VaultError> {
    let current_time = time();
    let mut session = with_payment_session(session_id, |s| Ok(s.clone()))?; // Clone to modify later

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
            if current_time > session.expires_at {
                session.state = PayState::Expired;
                session.error_message = Some("Session expired before verification.".to_string());
                store_payment_session(session);
                return Err(VaultError::PaymentError("Payment session has expired.".to_string()));
            }
            // Proceed to verification logic based on method
        }
        _ => {
            return Err(VaultError::InternalError(format!(
                "Verification encountered unexpected session state: {:?}",
                session.state
            )));
        }
    }

    // 2. Verification Logic based on Payment Method
    let verification_result = match session.method {
        PayMethod::IcpDirect => {
            verify_icp_ledger_payment(&session).await
        }
        PayMethod::ChainFusion => {
            verify_chainfusion_payment(&session).await
        }
    };

    // 3. Process Verification Result
    match verification_result {
        Ok(tx_hash) => {
            ic_cdk::print(format!(
                "✅ INFO: Payment verified for session {} (Tx: {})",
                session_id, tx_hash
            ));

            // Update session state
            session.state = PayState::Confirmed;
            session.verified_at = Some(current_time);
            session.ledger_tx_hash = Some(tx_hash.clone()); // Store the confirmed TX hash
            session.error_message = None;

            // Persist the confirmed session state BEFORE updating the vault
            store_payment_session(session.clone());

            // Update Vault Status (Placeholder: Needs actual VaultService call)
            // TODO: Integrate with VaultService to update vault status
            ic_cdk::print(format!(
                "✅ INFO: Vault {} status should be updated to NEED_SETUP (via VaultService).",
                vault_id
            ));
            // vault_service::set_vault_status(vault_id, VaultStatus::NeedSetup).await?;

            // Add Billing Entry (Placeholder: Needs actual BillingService call)
            // TODO: Integrate with BillingService to add an entry
            ic_cdk::print(format!(
                "✅ INFO: Billing entry should be added for vault {} (Tx: {}, Method: {:?}).",
                vault_id, tx_hash, session.method
            ));
            // let billing_entry = BillingEntry { ... };
            // crate::storage::billing::add_billing_entry(billing_entry)?;

            Ok(format!("Payment Confirmed (Tx: {})", tx_hash))
        }
        Err(e) => {
            ic_cdk::eprintln!(
                "🔥 ERROR: Payment verification failed for session {}: {:?}",
                session_id, e
            );
            // Store error state if it's a definite failure (not just pending)
            if !matches!(e, VaultError::PaymentError(_)) {
                session.state = PayState::Error;
                session.error_message = Some(format!("{:?}", e));
                store_payment_session(session);
            }
            Err(e) // Return the specific verification error
        }
    }
}

/// Specific logic to verify ICP direct payments against the ledger.
async fn verify_icp_ledger_payment(session: &PaymentSession) -> Result<String, VaultError> {
    ic_cdk::print(format!(
        "🔍 DEBUG: Verifying ICP Ledger for session {} (Amount: {}, To Account: {})",
        session.session_id, session.amount_e8s, session.pay_to_account_id
    ));

    let ledger_canister_id = Principal::from_text(ICP_LEDGER_CANISTER_ID_STR).expect("Invalid Ledger ID");
    let target_account = AccountIdentifier::from_hex(&session.pay_to_account_id)
        .map_err(|_| VaultError::InternalError("Invalid account identifier stored in session".to_string()))?;

    // --- PLACEHOLDER: Replace with actual ledger query --- 
    // TODO: Implement robust ledger query (e.g., `query_blocks` or `get_account_transactions` if available)
    // Checking balance is NOT sufficient proof of payment for a specific session.
    let payment_found_and_valid = true; // Simulate success
    let tx_hash_placeholder = format!("simulated_icp_tx_{}", session.session_id);
    // --- END PLACEHOLDER --- 

    if payment_found_and_valid {
        Ok(tx_hash_placeholder)
    } else {
        Err(VaultError::PaymentError("ICP payment not confirmed on ledger yet.".to_string()))
    }
}

/// Specific logic to verify ChainFusion payments.
async fn verify_chainfusion_payment(session: &PaymentSession) -> Result<String, VaultError> {
    ic_cdk::print(format!(
        "🔍 DEBUG: Verifying ChainFusion swap for session {}",
        session.session_id
    ));

    // 1. Call ChainFusion Adapter to get swap status
    let cf_status_resp = check_chainfusion_swap_status(&session.session_id).await?;

    match cf_status_resp.status {
        ChainFusionSwapStatus::Completed => {
            ic_cdk::print(format!(
                "🔗 INFO: ChainFusion swap completed for session {}. ICP Tx: {}",
                session.session_id,
                cf_status_resp.icp_tx_hash.as_deref().unwrap_or("N/A")
            ));

            // 2. IMPORTANT: Even if CF says completed, verify the ICP tx landed in our subaccount
            // Re-use the ICP ledger verification logic, but use the Tx hash from CF if available.
            // TODO: Refine ICP ledger verification to potentially use the provided Tx hash
            // For now, assume if CF says completed, the ICP tx is valid (this is NOT safe for production)
            let icp_tx_hash = cf_status_resp.icp_tx_hash.unwrap_or_else(|| format!("cf_completed_{}", session.session_id));

            // --- PLACEHOLDER: Verify ICP Tx from CF --- 
            let icp_tx_verified = true; // Assume verified
            // --- END PLACEHOLDER --- 

            if icp_tx_verified {
                Ok(icp_tx_hash)
            } else {
                Err(VaultError::PaymentError(
                    "ChainFusion swap completed, but ICP transaction verification failed.".to_string(),
                ))
            }
        }
        ChainFusionSwapStatus::Pending | ChainFusionSwapStatus::Processing => {
            Err(VaultError::PaymentError("ChainFusion swap is still processing.".to_string()))
        }
        ChainFusionSwapStatus::Expired => {
            Err(VaultError::PaymentError("ChainFusion swap offer expired.".to_string()))
        }
        ChainFusionSwapStatus::Failed(reason) => {
            Err(VaultError::PaymentError(format!(
                "ChainFusion swap failed: {}",
                reason
            )))
        }
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

// TODO: Implement actual billing log query
pub async fn list_billing_entries(offset: u64, limit: usize) -> Result<(Vec<BillingEntry>, u64), VaultError> {
    ic_cdk::print(format!(
        "🧾 INFO: Listing billing entries (offset: {}, limit: {}). Placeholder implementation.",
        offset,
        limit
    ));
    // Placeholder: Return empty list and zero total
    // Replace with actual stable log query
    Ok((Vec::new(), 0))
}

// --- Internal Helpers ---
// ... rest of file ...