// src/backend/services/payment_service.rs
use crate::{
    error::VaultError,
    models::{
        common::*, // Common types like Timestamp, PrincipalId, VaultId
        payment::*, // Payment models like PaymentSession, PayMethod, PayState, E8s
        billing::BillingEntry, // Import BillingEntry definition
        common::VaultStatus, // Needed for setting status
    },
    // Use storage module helpers for payment session (now defined in models::payment)
    models::payment::{store_payment_session, with_payment_session, with_payment_session_mut}, // Import storage helpers from models
    utils::crypto::generate_unique_principal, // For SessionId (Principal)
    adapter::chainfusion_adapter::{initialize_chainfusion_swap, check_chainfusion_swap_status, ChainFusionInitRequest, ChainFusionSwapStatus},
    services::vault_service, // Correct path for vault_service
    storage, // Import storage module for billing call
};
use candid::Principal;
use ic_cdk::api::{self, time}; 
use std::time::Duration;
// Use types directly from ic_ledger_types
use ic_ledger_types::{AccountIdentifier, Subaccount, AccountBalanceArgs, DEFAULT_SUBACCOUNT, transfer, TransferArgs, Memo, Tokens};
use crate::adapter::chainfusion_adapter; // Keep this one
// Removed unused ledger_adapter and account_identifier imports
use ic_cdk::api::management_canister::main::raw_rand;
// use crate::models::payment::PayState; // Already imported via payment::*

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
// Derives a unique random subaccount.
async fn derive_random_subaccount() -> Result<Subaccount, VaultError> {
    let rand_result: Result<(Vec<u8>,), _> = raw_rand().await; // Get Vec<u8>
    let rand_bytes_vec = rand_result.map_err(|(code, msg)| {
        VaultError::InternalError(format!("Failed to get random bytes for subaccount: [{:?}] {}", code, msg))
    })?.0; // Extract Vec<u8> from tuple

    // Try converting Vec<u8> to [u8; 32]
    let rand_bytes_array: [u8; 32] = rand_bytes_vec.try_into().map_err(|v: Vec<u8>| {
        VaultError::InternalError(format!("Failed to convert random bytes Vec (len {}) to [u8; 32]", v.len()))
    })?;

    Ok(Subaccount(rand_bytes_array))
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
    let session_id = generate_unique_principal().await?; 
    let current_time = time();
    let expires_at = current_time + Duration::from_secs(PAYMENT_SESSION_TIMEOUT_SECONDS).as_nanos() as u64;

    let vault_canister_principal = api::id();
    let subaccount = derive_random_subaccount().await?; // Use random subaccount
    let pay_to_account = AccountIdentifier::new(&vault_canister_principal, &subaccount);

    let mut session = PaymentSession {
        session_id: session_id.clone(),
        pay_to_account_id: pay_to_account.to_string(),
        pay_to_subaccount: Some(subaccount.0), // Store the subaccount bytes
        amount_e8s: req.amount_e8s,
        vault_plan: req.vault_plan.clone(), // Clone plan string
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

    // Store the session using the function from models::payment
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
    session_id: &PrincipalId,
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
            store_payment_session(session.clone()); // Use helper from models::payment

            // Update Vault Status
            ic_cdk::print(format!("INFO: Attempting to update vault {} status to NeedSetup...", vault_id)); // Use print macro
            match vault_service::set_vault_status(&vault_id.clone(), VaultStatus::NeedSetup, Some(session.session_id)).await {
                Ok(_) => ic_cdk::print("INFO: Vault status updated successfully.".to_string()), // Use print macro
                Err(e) => {
                    ic_cdk::eprintln!("ERROR: Failed to update vault status for {}: {:?}. Payment session {} remains Confirmed.", vault_id, e, session_id);
                }
            }

            // Add Billing Entry
            ic_cdk::print(format!("INFO: Adding billing entry for vault {}...", vault_id)); // Use print macro
            let billing_entry = BillingEntry {
                date: current_time, // Use 'date' field
                vault_id: vault_id.to_string(), // Convert Principal to String
                tx_type: "Vault Creation".to_string(), // Example tx_type
                amount_icp_e8s: session.amount_e8s, // Use u64
                original_token: session.chainfusion_source_token.clone(),
                original_amount: session.chainfusion_source_amount.clone(),
                payment_method: format!("{:?}", session.method), // Convert enum to String
                ledger_tx_hash: Some(tx_hash.clone()), // Use correct field name
                swap_tx_hash: None, // Populate if CF payment verification provides it
                related_principal: Some(session.initiating_principal), // Optional: store who paid
            };
            match storage::billing::add_billing_entry(billing_entry) { // Use correct storage path
                Ok(log_index) => ic_cdk::print(format!("INFO: Billing entry added at index {}.", log_index)), // Pass log_index
                Err(e) => {
                    ic_cdk::eprintln!("ERROR: Failed to add billing entry for vault {}: {}. Payment session {} remains Confirmed.", vault_id, e, session_id);
                }
            }

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
                store_payment_session(session); // Use helper from models::payment
            }
            Err(e) // Return the specific verification error
        }
    }
}

/// Specific logic to verify ICP direct payments against the ledger.
async fn verify_icp_ledger_payment(session: &PaymentSession) -> Result<String, VaultError> {
    ic_cdk::print(format!(
        "INFO: Verifying ICP ledger payment for session {} to account {}",
        session.session_id,
        session.pay_to_account_id
    ));

    let ledger_canister_id = Principal::from_text(ICP_LEDGER_CANISTER_ID_STR)
        .map_err(|_| VaultError::InternalError("Invalid ICP Ledger Canister ID configured".to_string()))?;

    // Reconstruct the AccountIdentifier from canister ID and stored subaccount
    let canister_principal = api::id();
    let subaccount = match session.pay_to_subaccount {
        Some(bytes) => Subaccount(bytes),
        None => return Err(VaultError::InternalError("Subaccount missing in payment session".to_string()))
    };
    let account = AccountIdentifier::new(&canister_principal, &subaccount);
    let args = AccountBalanceArgs { account };

    ic_cdk::print(format!("INFO: Querying balance for account: {}", account.to_hex())); // Use print macro

    // Call the ledger canister's account_balance method
    let balance_result: Result<(Tokens,), _> = ic_cdk::call(ledger_canister_id, "account_balance", (args,)).await;

    match balance_result {
        Ok((balance,)) => {
            ic_cdk::print(format!(
                "INFO: Account {} balance: {} e8s. Required: {} e8s.",
                account.to_hex(),
                balance.e8s(),
                session.amount_e8s
            )); // Use print macro
            // Check if the balance is sufficient
            if balance.e8s() >= session.amount_e8s {
                let confirmation_detail = format!("balance_confirmed_{}", balance.e8s());
                Ok(confirmation_detail)
            } else {
                Err(VaultError::PaymentError("Payment amount not found on ledger.".to_string()))
            }
        }
        Err((code, msg)) => {
            ic_cdk::eprintln!(
                "ERROR: Failed to query account balance from ICP ledger ({:?}): {}",
                code, msg
            );
            Err(VaultError::PaymentError(format!(
                "Ledger query failed: {}",
                msg
            )))
        }
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
            // We don't directly use the hash from CF for primary verification, but we do store it.
            // Primary verification remains checking the balance of the target subaccount.
            let audit_tx_hash = cf_status_resp.icp_tx_hash.unwrap_or_else(|| format!("cf_completed_{}", session.session_id));

            // Call the existing ICP ledger verification function
            match verify_icp_ledger_payment(session).await {
                Ok(_) => {
                    // Balance confirmed, return the tx hash provided by CF (or generated one) for auditing/logging
                    Ok(audit_tx_hash)
                }
                Err(e) => {
                     ic_cdk::eprintln!(
                        "🔥 ERROR: ChainFusion reported completed swap for session {}, but ICP balance check failed: {:?}",
                        session.session_id, e
                    );
                    // Even though CF said completed, our ledger check failed. Treat as payment error.
                     Err(VaultError::PaymentError(
                         "ChainFusion swap completed, but ICP transaction verification failed.".to_string(),
                     ))
                }
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

/// Closes a payment session, typically after it has been successfully used or expired.
pub fn close_payment_session(session_id: &PrincipalId) -> Result<(), VaultError> {
    let current_time = time();
    with_payment_session_mut(session_id, |session| { // Use helper from models::payment
        // Only close sessions that are Confirmed or Expired
        match session.state {
            PayState::Confirmed | PayState::Expired => {
                session.state = PayState::Closed;
                session.closed_at = Some(current_time);
                ic_cdk::print(format!("INFO: Payment session {} closed.", session_id)); // Use print macro
                Ok(())
            }
            PayState::Closed => {
                // Already closed, idempotent operation
                Ok(())
            }
            PayState::Issued => {
                Err(VaultError::PaymentError(format!(
                    "Cannot close active payment session {}. Verify or let expire first.",
                    session_id
                )))
            }
            PayState::Error => {
                Err(VaultError::PaymentError(format!(
                    "Cannot close payment session {} in error state.",
                    session_id
                )))
            }
             PayState::Pending => { // Added missing Pending state handling
                 Err(VaultError::PaymentError(format!(
                     "Cannot close payment session {} while payment is pending.",
                     session_id
                 )))
             }
        }
    })
}

/// Placeholder for listing billing entries (admin only)
pub async fn list_billing_entries(offset: usize, limit: usize) -> Result<(Vec<BillingEntry>, u64), VaultError> {
    // Use storage::billing helper functions
    let entries = storage::billing::query_billing_entries(offset, limit);
    let total = storage::billing::get_billing_log_len(); // Use helper for total count
    Ok((entries, total))
}

// --- Function to Get Session Status ---

/// Represents the publicly queryable status of a payment session.
#[derive(Clone, Debug, candid::CandidType, serde::Deserialize)]
pub struct PaymentSessionStatus {
   pub session_id: PrincipalId,
   pub state: PayState,
   pub error_message: Option<String>,
   pub verified_at: Option<Timestamp>,
   pub closed_at: Option<Timestamp>,
}

/// Retrieves the current status of a payment session.
pub fn get_payment_session_status(session_id: &PrincipalId) -> Result<PaymentSessionStatus, VaultError> {
   with_payment_session(session_id, |session| { // Use helper from models::payment
       Ok(PaymentSessionStatus {
           session_id: session.session_id.clone(),
           state: session.state,
           error_message: session.error_message.clone(),
           verified_at: session.verified_at,
           closed_at: session.closed_at,
       })
   })
}

// --- Internal Helpers ---
// ... rest of file ...