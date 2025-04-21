// src/backend/services/payment_service.rs
use crate::{
    error::VaultError,
    models::{
        common::*, // Common types like Timestamp, PrincipalId
        payment::*, // Payment models like PaymentSession, PayMethod, PayState, E8s, SessionId
        vault_service, // To potentially trigger vault status change
    },
    storage::payment::{store_payment_session, with_payment_session, with_payment_session_mut}, // Use storage module helpers
    utils::crypto::generate_unique_principal, // For SessionId
    adapter::chainfusion_adapter::{initialize_chainfusion_swap, check_chainfusion_swap_status, ChainFusionInitRequest, ChainFusionSwapStatus},
    services::vault_service, // Fix vault_service import
};
use candid::Principal;
use ic_cdk::api::{self, time};
use std::time::Duration;
use ic_ledger_types::{AccountIdentifier, Subaccount, AccountBalanceArgs, DEFAULT_SUBACCOUNT, transfer, TransferArgs, Memo, Tokens};
use crate::utils::account_identifier::AccountIdentifier;
use crate::adapter::chainfusion_adapter;
use crate::adapter::ledger_adapter; // Placeholder import
use ic_cdk::api::caller as ic_caller;
use crate::models::billing::BillingEntry;
use ic_cdk::api::management_canister::main::raw_rand;
use crate::models::payment::PayState; // Import PayState
use crate::models::common::VaultStatus; // Needed for setting status
use crate::storage; // Import storage module for billing call

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
    let (rand_bytes,): ([u8; 32],) = raw_rand().await.map_err(|(code, msg)| {
        VaultError::InternalError(format!("Failed to get random bytes for subaccount: [{:?}] {}", code, msg))
    })?;
    Ok(Subaccount(rand_bytes))
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
            store_payment_session(session.clone());

            // Update Vault Status (Placeholder: Needs actual VaultService call)
            ic_cdk::println!("INFO: Attempting to update vault {} status to NeedSetup...", vault_id);
            match vault_service::set_vault_status(vault_id.clone(), VaultStatus::NeedSetup).await {
                Ok(_) => ic_cdk::println!("INFO: Vault status updated successfully."),
                Err(e) => {
                    // Log error but don't necessarily fail the entire verification?
                    // The payment is verified, but vault update failed.
                    ic_cdk::eprintln!("ERROR: Failed to update vault status for {}: {:?}. Payment session {} remains Confirmed.", vault_id, e, session_id);
                    // Consider how to handle this - maybe retry later? For now, log and continue.
                }
            }

            // Add Billing Entry (Placeholder: Needs actual BillingService call)
            ic_cdk::println!("INFO: Adding billing entry for vault {}...", vault_id);
            let billing_entry = BillingEntry {
                timestamp: current_time,
                vault_id: vault_id.clone(), // Assuming BillingEntry takes Principal
                payment_method: session.method,
                amount_icp_e8s: Some(session.amount_e8s),
                original_token: session.chainfusion_source_token.clone(),
                original_amount: session.chainfusion_source_amount, 
                transaction_id: Some(tx_hash.clone()),
                swap_tx_hash: None, // Populate if CF payment verification provides it
                description: format!("Vault creation/plan: {}", session.vault_plan),
            };
            match storage::add_billing_entry(billing_entry) {
                Ok(log_index) => ic_cdk::println!("INFO: Billing entry added at index {}."),
                Err(e) => {
                    // Log error, but payment is already verified.
                    ic_cdk::eprintln!("ERROR: Failed to add billing entry for vault {}: {}. Payment session {} remains Confirmed.", vault_id, e, session_id);
                    // Consider implications - payment confirmed but not logged.
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
                store_payment_session(session);
            }
            Err(e) // Return the specific verification error
        }
    }
}

/// Specific logic to verify ICP direct payments against the ledger.
async fn verify_icp_ledger_payment(session: &PaymentSession) -> Result<String, VaultError> {
    ic_cdk::println!(
        "INFO: Verifying ICP ledger payment for session {} to account {}",
        session.session_id,
        session.pay_to_account_id
    );

    let ledger_canister_id = Principal::from_text(ICP_LEDGER_CANISTER_ID_STR)
        .map_err(|_| VaultError::InternalError("Invalid ICP Ledger Canister ID configured".to_string()))?;

    // Parse the pay_to_account_id string back into an AccountIdentifier
    let account: AccountIdentifier = session.pay_to_account_id.parse()
        .map_err(|e| VaultError::InternalError(format!("Failed to parse payment account identifier '{}': {}", session.pay_to_account_id, e)))?;

    let args = AccountBalanceArgs { account };

    ic_cdk::println!("INFO: Querying balance for account: {}", account.to_hex());

    // Call the ledger canister's account_balance method
    let balance_result: Result<(Tokens,), _> = ic_cdk::call(ledger_canister_id, "account_balance", (args,)).await;

    match balance_result {
        Ok((balance,)) => {
            ic_cdk::println!(
                "INFO: Account {} balance: {} e8s. Required: {} e8s.",
                account.to_hex(),
                balance.e8s(),
                session.amount_e8s
            );
            // Check if the balance is sufficient
            if balance.e8s() >= session.amount_e8s {
                // Payment confirmed. We don't get a specific TX hash this way,
                // so generate a confirmation string.
                // TODO: Refine ICP ledger verification to potentially use the provided Tx hash
                // (This isn't easily possible with just account_balance. Would need user input
                // or a more complex query if the ledger supported it).
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

/// Closes a payment session, typically after it has been successfully used or expired.
pub fn close_payment_session(session_id: &PrincipalId) -> Result<(), VaultError> {
    let current_time = time();
    with_payment_session_mut(session_id, |session| {
        // Only close sessions that are Confirmed or Expired
        match session.state {
            PayState::Confirmed | PayState::Expired => {
                session.state = PayState::Closed;
                session.closed_at = Some(current_time);
                ic_cdk::println!("INFO: Payment session {} closed.", session_id);
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
        }
    })
}

/// Placeholder for listing billing entries (admin only)
// TODO: Implement actual billing log query using storage::billing
pub async fn list_billing_entries(offset: usize, limit: usize) -> Result<(Vec<BillingEntry>, u64), VaultError> {
    // Use storage::billing::query_billing_entries or similar
    let entries = crate::storage::billing::query_billing_entries(offset, limit);
    let total = crate::storage::billing::BILLING_LOG.with(|log| log.borrow().len()); // Get total count
    Ok((entries, total))
}

// TODO: Add function to handle ChainFusion payments (initiate swap, verify swap completion)
// TODO: Add function to get payment session status

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
   with_payment_session(session_id, |session| {
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