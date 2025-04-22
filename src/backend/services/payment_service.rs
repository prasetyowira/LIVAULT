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
    services::vault_service, // Correct path for vault_service
    storage, // Import storage module for billing call
};
use candid::Principal;
use ic_cdk::api::time;
use std::time::Duration;
// Use types directly from ic_ledger_types
use ic_ledger_types::{AccountIdentifier, Subaccount, AccountBalanceArgs, DEFAULT_SUBACCOUNT, transfer, TransferArgs, Memo, Tokens};
// use crate::adapter::chainfusion_adapter; // REMOVED
// Removed unused ledger_adapter and account_identifier imports
use ic_cdk::api::management_canister::main::raw_rand;
// use crate::models::payment::PayState; // Already imported via payment::*
use candid::{CandidType, Decode, Deserialize, Encode, Nat};
use ic_cdk::api::{self, call::call_with_payment};
use ic_ledger_types::{ // Import types from the crate
    BlockIndex,
    GetBlocksArgs,
    QueryBlocksResponse,
    Operation
    // EncodedBlock is not directly needed as we decode
    // Block as well if needed after decoding
};

// Constants
const PAYMENT_SESSION_TIMEOUT_SECONDS: u64 = 30 * 60; // 30 minutes
const ICP_LEDGER_CANISTER_ID_STR: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai"; // Mainnet ICP Ledger

// --- Payment Initialization Struct (from API) ---
#[derive(Clone, Debug, candid::CandidType, serde::Deserialize)]
pub struct PaymentInitRequest {
    pub vault_plan: String,
    pub amount_e8s: E8s,
    // method: PayMethod, // REMOVED - Always IcpDirect for MVP
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
    // Ensure method is IcpDirect (only option left) - No longer needed to check req.method
    // if req.method != PayMethod::IcpDirect {
    //     return Err(VaultError::InvalidInput("Only IcpDirect payment method is supported for MVP.".to_string()));
    // }

    let session_id = generate_unique_principal().await?; 
    let current_time = time();
    let expires_at = current_time + Duration::from_secs(PAYMENT_SESSION_TIMEOUT_SECONDS).as_nanos() as u64;

    let vault_canister_principal = api::id();
    let subaccount = derive_random_subaccount().await?; // Use random subaccount
    let pay_to_account = AccountIdentifier::new(&vault_canister_principal, &subaccount);

    let session = PaymentSession {
        session_id: session_id.clone(),
        pay_to_account_id: pay_to_account.to_string(),
        pay_to_subaccount: Some(subaccount.0), // Store the subaccount bytes
        amount_e8s: req.amount_e8s,
        vault_plan: req.vault_plan.clone(), // Clone plan string
        method: PayMethod::IcpDirect, // Explicitly set to IcpDirect
        state: PayState::Issued,
        initiating_principal: caller,
        created_at: current_time,
        expires_at,
        verified_at: None,
        closed_at: None,
        error_message: None,
        ledger_tx_hash: None,
        // Removed ChainFusion specific fields initialization
    };

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
/// * `block_index` - Optional block index provided by the frontend where the transaction is expected.
///
/// # Returns
/// * `Result<String, VaultError>` - Confirmation message (e.g., "Payment Confirmed (Block: 123)") or an error.
pub async fn verify_payment(
    session_id: &PrincipalId,
    vault_id: &VaultId, // Used to update vault status upon confirmation
    block_index: Option<u64>, // Add block_index parameter
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
        PayState::Issued | PayState::Pending => { // Allow verification if Issued or Pending
            if current_time > session.expires_at {
                session.state = PayState::Expired;
                session.error_message = Some("Session expired before verification.".to_string());
                store_payment_session(session); // Use helper from models::payment
                return Err(VaultError::PaymentError("Payment session has expired.".to_string()));
            }
            // Proceed to verification logic
            // Update state to Pending if it was Issued
            if session.state == PayState::Issued {
                session.state = PayState::Pending;
                store_payment_session(session.clone()); // Store pending state
            }
        }
        _ => {
            return Err(VaultError::InternalError(format!(
                "Verification encountered unexpected session state: {:?}",
                session.state
            )));
        }
    }

    // 2. Verification Logic using specific block index if provided
    let verification_result = verify_icp_ledger_payment(&session, block_index).await;

    // 3. Process Verification Result
    match verification_result {
        Ok(confirmation_detail) => { // Renamed tx_hash to confirmation_detail
            ic_cdk::print(format!(
                "✅ INFO: Payment verified for session {} ({})",
                session_id, confirmation_detail
            ));

            // Update session state
            session.state = PayState::Confirmed;
            session.verified_at = Some(current_time);
            session.ledger_tx_hash = Some(confirmation_detail.clone()); // Store the confirmation detail (e.g., block_123)
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
                payment_method: format!("{:?}", PayMethod::IcpDirect), // Always IcpDirect for MVP
                ledger_tx_hash: Some(confirmation_detail.clone()), // Use correct field name
                related_principal: Some(session.initiating_principal), // Optional: store who paid
            };
            match storage::billing::add_billing_entry(billing_entry) { // Use correct storage path
                Ok(log_index) => ic_cdk::print(format!("INFO: Billing entry added at index {}.", log_index)), // Pass log_index
                Err(e) => {
                    ic_cdk::eprintln!("ERROR: Failed to add billing entry for vault {}: {}. Payment session {} remains Confirmed.", vault_id, e, session_id);
                }
            }

            Ok(format!("Payment Confirmed ({})", confirmation_detail))
        }
        Err(e) => {
            ic_cdk::eprintln!(
                "🔥 ERROR: Payment verification failed for session {}: {:?}",
                session_id, e
            );
            // Store error state if it's a definite failure (not just pending)
            // Keep session as Pending if verification just failed temporarily
            if !matches!(e, VaultError::PaymentError(_)) { 
                session.state = PayState::Error;
                session.error_message = Some(format!("{:?}", e));
            } else {
                 // If it was just a PaymentError (e.g., tx not found yet), keep as Pending
                 session.state = PayState::Pending; 
                 session.error_message = Some(e.to_string()); // Store the temporary error
            }
            store_payment_session(session); // Store updated state (Error or back to Pending)
            Err(e) // Return the specific verification error
        }
    }
}

/// Specific logic to verify ICP direct payments against the ledger using query_blocks.
/// Prioritizes checking a specific block index if provided.
async fn verify_icp_ledger_payment(session: &PaymentSession, block_index_opt: Option<u64>) -> Result<String, VaultError> {
    ic_cdk::print(format!(
        "INFO: Verifying ICP ledger payment for session {} to account {}. Checking block: {:?}",
        session.session_id,
        session.pay_to_account_id,
        block_index_opt
    ));

    let ledger_canister_id = Principal::from_text(ICP_LEDGER_CANISTER_ID_STR)
        .map_err(|_| VaultError::InternalError("Invalid ICP Ledger Canister ID configured".to_string()))?;

    // 1. Determine Target AccountIdentifier (using ic_ledger_types::AccountIdentifier)
    let target_account = AccountIdentifier::from_hex(&session.pay_to_account_id)
        .map_err(|e| VaultError::InternalError(format!("Failed to parse target account ID: {}", e)))?;

    // 2. Prepare query_blocks arguments (using ic_ledger_types::GetBlocksArgs)
    let query_args = if let Some(index) = block_index_opt {
        GetBlocksArgs { start: index, length: 1 }
    } else {
        // Fallback logic remains the same, resulting in GetBlocksArgs
        ic_cdk::print("WARN: No specific block index provided, falling back to querying recent blocks.");
        const MAX_BLOCKS_TO_QUERY: u64 = 100;
        let get_len_args = GetBlocksArgs { start: 0, length: 0 };
        let chain_len_result: Result<(QueryBlocksResponse,), _> = ic_cdk::call(ledger_canister_id, "query_blocks", (get_len_args,)).await;
        let chain_length = match chain_len_result {
            Ok((resp,)) => resp.chain_length,
            Err((code, msg)) => {
                ic_cdk::eprintln!("ERROR: Failed to query ledger chain_length ({:?}): {}", code, msg);
                return Err(VaultError::PaymentError(format!("Ledger query_blocks (length) failed: {}", msg)));
            }
        };
        let start_block = chain_length.saturating_sub(MAX_BLOCKS_TO_QUERY);
        let query_length = std::cmp::min(MAX_BLOCKS_TO_QUERY, chain_length);
        GetBlocksArgs { start: start_block, length: query_length }
    };

    ic_cdk::print(format!(
        "INFO: Querying ledger blocks from {} (length {})",
        query_args.start, query_args.length
    ));

    // 3. Call query_blocks (response type is now ic_ledger_types::QueryBlocksResponse)
    let blocks_result: Result<(QueryBlocksResponse,), _> = ic_cdk::call(ledger_canister_id, "query_blocks", (query_args.clone(),)).await;

    match blocks_result {
        Ok((response,)) => {
            ic_cdk::print(format!(
                "INFO: Received {} blocks starting from index {}. Total chain length: {}.",
                response.blocks.len(), response.first_block_index, response.chain_length
            ));

            if response.blocks.is_empty() {
                 return Err(VaultError::PaymentError(format!(
                     "Ledger query returned no blocks for range starting at {}.",
                     query_args.start
                 )));
            }

            // 4. Iterate through blocks (Vec<EncodedBlock>) and decode each one
            for (i, block) in response.blocks.iter().enumerate() {
                let current_block_index = response.first_block_index + i as u64;
                // If a specific index was requested, ensure we are looking at the correct block
                if block.is_some() && block_index_opt != Some(current_block_index) {
                    continue;
                }

                // Access decoded transaction details (using ic_ledger_types::Transaction and Operation)
                if let Some(Operation::Transfer { to, amount, .. }) = block.transaction.operation {
                    let tx_timestamp_nanos = block.transaction.created_at_time.timestamp_nanos;

                    // Check if the transaction matches the session criteria
                    if to == target_account // Comparison works directly with ic_ledger_types::AccountIdentifier
                        && amount.e8s() >= session.amount_e8s // Use get_e8s() for ic_ledger_types::Tokens
                        && tx_timestamp_nanos >= session.created_at
                        && tx_timestamp_nanos <= session.expires_at
                    {
                        ic_cdk::print(format!(
                            "✅ Found matching transaction in block {}: to={}, amount={}e8s, time={}",
                            current_block_index, to, amount.e8s(), tx_timestamp_nanos
                        ));
                        let confirmation_detail = format!("block_{}", current_block_index);
                        return Ok(confirmation_detail);
                    }
                } else {
                     ic_cdk::print(format!("DEBUG: Block {} does not contain a Transfer operation.", current_block_index));
                }
            }

            // 5. No matching transaction found in the queried block(s)
            let block_range_str = if let Some(index) = block_index_opt {
                 format!("block {}", index)
             } else {
                 format!("blocks {}-{}", response.first_block_index, response.first_block_index + response.blocks.len() as u64 -1)
             };
             ic_cdk::print(format!(
                 "INFO: No matching transaction found for session {} in {}",
                 session.session_id, block_range_str
             ));
             Err(VaultError::PaymentError(
                 "Payment transaction not found in specified/recent ledger block(s).".to_string(),
             ))
        }
        Err((code, msg)) => {
            ic_cdk::eprintln!(
                "ERROR: Failed to query blocks from ICP ledger ({:?}): {}",
                code, msg
            );
            Err(VaultError::PaymentError(format!(
                "Ledger query_blocks failed: {}",
                msg
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