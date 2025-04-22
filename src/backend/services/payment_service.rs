// src/backend/services/payment_service.rs
use crate::{
    error::VaultError,
    models::{common::*, payment::*, billing::BillingEntry, common::VaultStatus},
    models::payment::{store_payment_session, with_payment_session, with_payment_session_mut},
    utils::crypto::generate_unique_principal,
    services::vault_service,
    storage,
};
use candid::{CandidType, Principal};
use ic_cdk::api::{self, time, management_canister::main::raw_rand};
use ic_ledger_types::{ // Import types from the crate
    AccountIdentifier,
    GetBlocksArgs,
    Operation,
    QueryBlocksResponse,
    Subaccount,
};
use std::time::Duration;

// Constants
const PAYMENT_SESSION_TIMEOUT_SECONDS: u64 = 30 * 60; // 30 minutes
const ICP_LEDGER_CANISTER_ID_STR: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai"; // Mainnet ICP Ledger

// --- Payment Initialization Struct (from API) ---
#[derive(Clone, Debug, CandidType, serde::Deserialize)]
pub struct PaymentInitRequest {
    pub vault_plan: String,
    pub amount_e8s: E8s,
}

// --- Helper: Derive Subaccount --- 
async fn derive_random_subaccount() -> Result<Subaccount, VaultError> {
    let rand_bytes_vec = raw_rand().await
        .map_err(|(code, msg)| VaultError::InternalError(format!("Failed to get random bytes for subaccount: [{:?}] {}", code, msg)))?.0;
    let rand_bytes_array: [u8; 32] = rand_bytes_vec.try_into()
        .map_err(|v: Vec<u8>| VaultError::InternalError(format!("Random bytes Vec (len {}) not convertible to [u8; 32]", v.len())))?;
    Ok(Subaccount(rand_bytes_array))
}

// --- Service Functions ---

/// Initializes a new payment session.
pub async fn initialize_payment_session(
    req: PaymentInitRequest,
    caller: PrincipalId, 
) -> Result<PaymentSession, VaultError> {
    let session_id = generate_unique_principal().await?; 
    let current_time = time();
    let expires_at = current_time + Duration::from_secs(PAYMENT_SESSION_TIMEOUT_SECONDS).as_nanos() as u64;

    let vault_canister_principal = api::id();
    let subaccount = derive_random_subaccount().await?;
    let pay_to_account = AccountIdentifier::new(&vault_canister_principal, &subaccount);

    let session = PaymentSession {
        session_id: session_id.clone(),
        pay_to_account_id: pay_to_account.to_string(),
        pay_to_subaccount: Some(subaccount.0),
        amount_e8s: req.amount_e8s,
        vault_plan: req.vault_plan.clone(),
        method: PayMethod::IcpDirect,
        state: PayState::Issued,
        initiating_principal: caller,
        created_at: current_time,
        expires_at,
        verified_at: None,
        closed_at: None,
        error_message: None,
        ledger_tx_hash: None,
    };

    store_payment_session(session.clone());

    ic_cdk::print(format!(
        "📝 INFO: Payment session {} created for plan {} ({} e8s) by {}. Method: {:?}. Expires at {}. Pay to Account: {}",
        session_id, session.vault_plan, session.amount_e8s, caller, session.method, session.expires_at, session.pay_to_account_id
    ));

    Ok(session)
}

/// Verifies if a payment matching the session details has been confirmed on the ICP Ledger.
pub async fn verify_payment(
    session_id: &PrincipalId,
    vault_id: &VaultId,
    block_index: Option<u64>,
) -> Result<String, VaultError> {
    let current_time = time();
    let mut session = with_payment_session(session_id, |s| Ok(s.clone()))?;

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
                "Payment session in error state: {}", session.error_message.unwrap_or_default()
            )));
        }
        PayState::Issued | PayState::Pending => {
            if session.is_expired(current_time) { // Use helper method
                session.state = PayState::Expired;
                session.error_message = Some("Session expired before verification.".to_string());
                store_payment_session(session);
                return Err(VaultError::PaymentError("Payment session has expired.".to_string()));
            }
            if session.state == PayState::Issued {
                session.state = PayState::Pending;
                store_payment_session(session.clone());
            }
        }
    }

    // 2. Verification Logic
    let verification_result = verify_icp_ledger_payment(&session, block_index).await;

    // 3. Process Verification Result
    match verification_result {
        Ok(confirmation_detail) => {
            ic_cdk::print(format!("✅ INFO: Payment verified for session {} ({})", session_id, confirmation_detail));

            session.state = PayState::Confirmed;
            session.verified_at = Some(current_time);
            session.ledger_tx_hash = Some(confirmation_detail.clone());
            session.error_message = None;
            store_payment_session(session.clone());

            // Trigger post-confirmation actions asynchronously
            trigger_post_confirmation_actions(vault_id.clone(), session.session_id, confirmation_detail, session.amount_e8s, session.initiating_principal);

            Ok(format!("Payment Confirmed ({})", session.ledger_tx_hash.unwrap_or_default()))
        }
        Err(e) => {
            ic_cdk::eprintln!("🔥 ERROR: Payment verification failed for session {}: {:?}", session_id, e);
            session.state = PayState::Pending; // Revert to Pending on verification failure
            session.error_message = Some(e.to_string());
            store_payment_session(session);
            Err(e)
        }
    }
}

/// Spawns async tasks for post-payment confirmation actions (update vault, add billing).
fn trigger_post_confirmation_actions(
    vault_id: VaultId,
    session_id: PrincipalId,
    confirmation_detail: String,
    amount_e8s: E8s,
    initiating_principal: PrincipalId
) {
    let vault_id_clone = vault_id.clone();
    let session_id_clone = session_id.clone();
    // Update Vault Status Task
    ic_cdk::spawn(async move {
        ic_cdk::print(format!("INFO: Attempting to update vault {} status to NeedSetup...", vault_id_clone));
        match vault_service::set_vault_status(&vault_id_clone, VaultStatus::NeedSetup, Some(session_id_clone)).await {
            Ok(_) => ic_cdk::print("INFO: Vault status updated successfully.".to_string()),
            Err(e) => ic_cdk::eprintln!("ERROR: Failed to update vault status for {}: {:?}. Payment session {} remains Confirmed.", vault_id_clone, e, session_id_clone),
        }
    });

    // Add Billing Entry Task
    ic_cdk::spawn(async move {
        ic_cdk::print(format!("INFO: Adding billing entry for vault {}...", vault_id));
        let billing_entry = BillingEntry {
            date: time(),
            vault_id: vault_id.to_string(),
            tx_type: "Vault Creation".to_string(),
            amount_icp_e8s: amount_e8s,
            payment_method: format!("{:?}", PayMethod::IcpDirect),
            ledger_tx_hash: Some(confirmation_detail),
            related_principal: Some(initiating_principal),
        };
        match storage::billing::add_billing_entry(billing_entry) {
            Ok(log_index) => ic_cdk::print(format!("INFO: Billing entry added at index {}.", log_index)),
            Err(e) => ic_cdk::eprintln!("ERROR: Failed to add billing entry for vault {}: {}. Payment session {} remains Confirmed.", vault_id, e, session_id),
        }
    });
}

/// Verifies ICP payment by querying ledger blocks.
async fn verify_icp_ledger_payment(session: &PaymentSession, block_index_opt: Option<u64>) -> Result<String, VaultError> {
    ic_cdk::print(format!("INFO: Verifying ICP ledger payment for session {}. Checking block: {:?}", session.session_id, block_index_opt));

    let ledger_canister_id = Principal::from_text(ICP_LEDGER_CANISTER_ID_STR)
        .map_err(|_| VaultError::InternalError("Invalid ICP Ledger Canister ID configured".to_string()))?;

    let target_account = AccountIdentifier::from_hex(&session.pay_to_account_id)
        .map_err(|e| VaultError::InternalError(format!("Invalid target account ID: {}", e)))?;

    let query_args = if let Some(index) = block_index_opt {
        GetBlocksArgs { start: index, length: 1 }
    } else {
        ic_cdk::print("WARN: No specific block index provided for verification.");
        return Err(VaultError::PaymentError("Block index required for efficient verification.".to_string()));
        // Removed fallback logic - require block index for verification
    };

    ic_cdk::print(format!("INFO: Querying ledger block {}", query_args.start));

    let blocks_result: Result<(QueryBlocksResponse,), _> = ic_cdk::call(ledger_canister_id, "query_blocks", (query_args.clone(),)).await;

    match blocks_result {
        Ok((response,)) => {
            ic_cdk::print(format!("INFO: Received {} blocks starting from index {}.", response.blocks.len(), response.first_block_index));

            if response.blocks.is_empty() {
                return Err(VaultError::PaymentError(format!("Ledger query returned no block for index {}.", query_args.start)));
            }

            let block = response.blocks.first().unwrap(); // Safe unwrap due to is_empty check

            if let Some(Operation::Transfer { to, amount, .. }) = block.transaction.operation {
                let tx_timestamp_nanos = block.transaction.created_at_time.timestamp_nanos;
                if to == target_account
                    && amount.e8s() >= session.amount_e8s
                    && tx_timestamp_nanos >= session.created_at
                    && tx_timestamp_nanos <= session.expires_at
                {
                    ic_cdk::print(format!("✅ Found matching transaction in block {}: amount={}e8s", query_args.start, amount.e8s()));
                    let confirmation_detail = format!("block_{}", query_args.start);
                    return Ok(confirmation_detail);
                } else {
                    ic_cdk::print(format!("DEBUG: Transfer in block {} did not match session criteria (to={}, amount={}, time={}, session_expires={}).", 
                        query_args.start, to, amount.e8s(), tx_timestamp_nanos, session.expires_at));
                }
            } else {
                ic_cdk::print(format!("DEBUG: Block {} does not contain a Transfer operation.", query_args.start));
            }

            Err(VaultError::PaymentError("Payment transaction not found in specified block or did not match criteria.".to_string()))
        }
        Err((code, msg)) => {
            ic_cdk::eprintln!("ERROR: Failed to query block {} from ICP ledger ({:?}): {}", query_args.start, code, msg);
            Err(VaultError::PaymentError(format!("Ledger query_blocks failed: {}", msg)))
        }
    }
}

/// Closes a payment session.
pub fn close_payment_session(session_id: &PrincipalId) -> Result<(), VaultError> {
    let current_time = time();
    with_payment_session_mut(session_id, |session| {
        match session.state {
            PayState::Confirmed | PayState::Expired | PayState::Error => { // Allow closing Error state too
                session.state = PayState::Closed;
                session.closed_at = Some(current_time);
                ic_cdk::print(format!("INFO: Payment session {} closed.", session_id));
                Ok(())
            }
            PayState::Closed => Ok(()), // Idempotent
            PayState::Issued | PayState::Pending => Err(VaultError::PaymentError(format!(
                "Cannot close active/pending payment session {}. Verify/expire first.", session_id
            ))),
        }
    })
}

/// Lists billing entries (admin only).
pub async fn list_billing_entries(offset: usize, limit: usize) -> Result<(Vec<BillingEntry>, u64), VaultError> {
    let entries = storage::billing::query_billing_entries(offset, limit);
    let total = storage::billing::get_billing_log_len();
    Ok((entries, total))
}

// --- Function to Get Session Status ---

/// Represents the publicly queryable status of a payment session.
#[derive(Clone, Debug, CandidType, serde::Deserialize)]
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