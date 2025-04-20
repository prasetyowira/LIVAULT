// src/backend/api.rs
// Placeholder for Candid API endpoint definitions (query/update functions) 

use crate::{
    error::VaultError,
    metrics::VaultMetrics, // Import the correct VaultMetrics struct
    models::common::*,
    models::{VaultConfig, VaultInviteToken, VaultMember, UnlockConditions},
    services::{
        invite_service::{self, InviteClaimData, MemberProfile},
        upload_service::{self, FileMeta},
        vault_service::{self, VaultInitData, VaultUpdateData},
        payment_service::{self, PaymentInitRequest as PaymentServiceInitRequest},
        scheduler_service,
    },
    storage::get_metrics as get_stored_metrics, // Import storage helper
    utils::guards::{check_admin, check_cycles}, // Import guards
    utils::rate_limit::rate_guard, // Import the rate guard
    models::billing::BillingEntry, // Import BillingEntry
    storage::{VAULT_CONFIGS, BILLING_LOG, Cbor}, // Import storage structures
};
use candid::{CandidType, Deserialize, Principal, Nat}; // Import Nat
use ic_cdk::{caller, query, update, api};
use ic_cdk::api::{canister_balance128, data_certificate, set_certified_data}; // Import IC APIs
use ic_cdk_macros::query; // Use specific import for clarity
use crate::models::payment::PaymentSession; // Import PaymentSession for return type
use std::cell::RefCell;
use std::collections::HashMap;
use validator::{Validate, ValidationError}; // Assuming 'validator' crate is added or available

// --- Admin Principal (Example - Load from stable memory/config later) ---
thread_local! {
    // TODO: Replace Principal::anonymous() with loading admin from init args or stable storage
    static ADMIN_PRINCIPAL: RefCell<Principal> = RefCell::new(Principal::anonymous()); // Use anonymous as safer placeholder
    // TODO: Replace Principal::anonymous() with loading cron caller from init args or stable storage
    static CRON_CALLER_PRINCIPAL: RefCell<Principal> = RefCell::new(Principal::anonymous()); // Use anonymous as safer placeholder
}

// --- Guard Functions ---

/// Checks if the caller is the designated admin principal.
fn admin_guard() -> Result<(), VaultError> { // Return VaultError
    ADMIN_PRINCIPAL.with(|admin_ref| check_admin(*admin_ref.borrow()))
}

/// Checks if the caller is the designated cron trigger OR the admin principal.
fn cron_or_admin_guard() -> Result<(), VaultError> { // Return VaultError
    let caller = api::caller();
    let is_admin = ADMIN_PRINCIPAL.with(|admin_ref| *admin_ref.borrow() == caller);
    let is_cron = CRON_CALLER_PRINCIPAL.with(|cron_ref| *cron_ref.borrow() == caller);

    if is_admin || is_cron {
        Ok(())
    } else {
        Err(VaultError::NotAuthorized(
            "Caller is not the authorized cron trigger or an admin.".to_string(),
        ))
    }
}

// --- Validation Helper ---
fn validate_request<T: Validate>(req: &T) -> Result<(), VaultError> {
    req.validate().map_err(|e| {
        // Convert validation errors to a user-friendly string or use specific variants
        let errors = e.field_errors().iter().map(|(field, errors)| {
            format!("{}: {}", field, errors.iter().map(|err| err.code.to_string()).collect::<Vec<_>>().join(", "))
        }).collect::<Vec<_>>().join("; ");
        VaultError::InvalidInput(format!("Validation failed: {}", errors))
    })
}

// --- Request/Response Structs (as per icp-api-conventions) ---

// Vault Creation
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct CreateVaultRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(max = 500))]
    pub description: Option<String>,
    #[validate(length(min = 1))]
    pub plan: String,
    // unlock_conditions: Option<UnlockConditions>, // Set via update?
}

// Vault Update
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct UpdateVaultRequest {
    #[validate(length(min = 26, max = 26))]
    pub vault_id: VaultId,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(max = 500))]
    pub description: Option<String>,
    pub unlock_conditions: Option<UnlockConditions>,
    #[validate(length(min = 1))]
    pub plan: Option<String>,
}

// Generate Invite
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct GenerateInviteRequest {
    #[validate(length(min = 26, max = 26))]
    pub vault_id: VaultId,
    pub role: Role,
}

// Claim Invite
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct ClaimInviteRequest {
    #[validate(length(min = 1))]
    pub token: InviteTokenId,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub relation: Option<String>,
}

// Upload
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct BeginUploadRequest {
    #[validate(length(min = 26, max = 26))]
    pub vault_id: VaultId,
    #[validate]
    pub file_meta: FileMeta,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct UploadChunkRequest {
    pub upload_id: UploadId,
    pub chunk_index: u32,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct FinishUploadRequest {
    #[validate(length(min = 1))]
    pub upload_id: UploadId,
    #[validate(length(min = 64, max = 64))]
    pub sha256_checksum_hex: String,
}

// Download
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct RequestDownloadRequest {
    #[validate(length(min = 26, max = 26))]
    pub vault_id: VaultId,
    #[validate(length(min = 1))]
    pub content_id: ContentId,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct DownloadInfo {
    pub url: String, // Temporary URL (e.g., presigned)
    pub expires_at: Timestamp,
}

// Unlock
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct TriggerUnlockRequest {
    #[validate(length(min = 26, max = 26))]
    pub vault_id: VaultId,
    // Add any necessary witness data/proof if needed
}

// Admin & Listing
#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct ListRequest {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct ListVaultsResponse {
    pub vaults: Vec<VaultSummary>,
    pub total: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct ListBillingResponse {
     pub entries: Vec<BillingEntry>,
     pub total: u64,
}

// Define needed summary/entry structs for admin lists
#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct VaultSummary {
     pub vault_id: VaultId,
     pub owner: PrincipalId,
     pub status: VaultStatus,
     pub storage_used_bytes: u64,
     pub plan: String,
     pub created_at: Timestamp,
}

#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct BillingEntry {
     // Define fields based on architecture doc
     pub date: Timestamp,
     pub vault_id: VaultId,
     pub tx_type: String, // purchase, upgrade, etc.
     pub amount_e8s: u64,
     pub token: String, // ICP, ETH, etc.
     pub tx_hash: Option<String>,
}

// Define the response type for get_metrics including dynamic cycle balance
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct GetMetricsResponse {
    pub metrics: VaultMetrics,
    pub cycle_balance: Nat,
    // pub cycles_burn_per_day: Nat, // Calculation TBD
}

// --- Candid Endpoints ---

// --- Payment Endpoints (Phase 4) ---

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ApiPaymentInitRequest { // Renamed to avoid conflict if needed
    pub vault_plan: String,
    pub amount_e8s: u64, // E8s
    pub method: crate::models::payment::PayMethod,
}

#[update] // Payment initialization likely involves state change (session creation)
async fn init_payment(req: ApiPaymentInitRequest) -> Result<PaymentSession, VaultError> {
    validate_request(&req)?; // TODO: Add Validate derive to ApiPaymentInitRequest if needed
    // Apply rate limiting if desired
    // rate_guard(caller())?;
    // Apply cycle check
    check_cycles()?;

    let session_data = payment_service::PaymentInitRequest {
        vault_plan: req.vault_plan,
        amount_e8s: req.amount_e8s,
        method: req.method,
        caller: caller(),
    };
    payment_service::initialize_payment_session(session_data).await
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct VerifyPaymentRequest {
    pub session_id: SessionId,
    pub vault_id: VaultId, // Likely needed to link payment to vault
}

#[update] // Verification likely updates vault/session state
async fn verify_payment(req: VerifyPaymentRequest) -> Result<String, VaultError> {
    validate_request(&req)?; // TODO: Add Validate derive to VerifyPaymentRequest if needed
    // Apply cycle check
    check_cycles()?;
    payment_service::verify_payment(req.session_id, req.vault_id).await
}

// --- Vault Core Endpoints ---

#[update]
async fn create_vault(req: CreateVaultRequest) -> Result<VaultId, VaultError> {
    validate_request(&req)?; // Validate input
    // rate_guard(caller())?;
    check_cycles()?;

    let init_data = VaultInitData {
        owner: caller(),
        name: req.name,
        description: req.description,
        plan: req.plan,
    };
    vault_service::create_vault(init_data)
}

#[query] // Read-only access to vault data
async fn get_vault(vault_id: VaultId) -> Result<VaultConfig, VaultError> {
    // Add authorization check: Allow owner or members?
    // rate_guard(caller())?;
    vault_service::get_vault_config(&vault_id)
}

#[update]
async fn update_vault(req: UpdateVaultRequest) -> Result<(), VaultError> {
    validate_request(&req)?; // Validate input
    // rate_guard(caller())?;
    check_cycles()?;
    // Add authorization: Only owner can update
    let update_data = VaultUpdateData {
        name: req.name,
        description: req.description,
        unlock_conditions: req.unlock_conditions,
        plan: req.plan,
    };
    vault_service::update_vault(&req.vault_id, update_data, caller())
}

// --- Invitation & Member Endpoints ---

#[update]
async fn generate_invite(req: GenerateInviteRequest) -> Result<VaultInviteToken, VaultError> {
    validate_request(&req)?; // Validate input
    // rate_guard(caller())?;
    check_cycles()?;
    // Add authorization: Only owner can generate invites
    invite_service::generate_invite_token(&req.vault_id, req.role, caller())
}

#[update]
async fn claim_invite(req: ClaimInviteRequest) -> Result<MemberProfile, VaultError> {
    validate_request(&req)?; // Validate input
    // rate_guard(caller())?;
    check_cycles()?;
    let claim_data = InviteClaimData {
        token: req.token,
        name: req.name,
        relation: req.relation,
        claimer: caller(),
    };
    invite_service::claim_invite(claim_data)
}

// --- Content Upload Endpoints ---

#[update]
async fn begin_upload(req: BeginUploadRequest) -> Result<UploadId, VaultError> {
    validate_request(&req)?; // Validate input
    // rate_guard(caller())?;
    check_cycles()?;
    // Add authorization: Owner or member?
    // Add quota check
    upload_service::begin_chunked_upload(&req.vault_id, req.file_meta, caller())
}

#[update]
async fn upload_chunk(req: UploadChunkRequest) -> Result<(), VaultError> {
    // validate_request(&req)?; // Skip validation for raw chunk data
    // rate_guard(caller())?;
    // No cycle check per chunk? Might be too expensive. Check on begin/finish.
    upload_service::upload_chunk(req.upload_id, req.chunk_index, req.data, caller())
}

#[update]
async fn finish_upload(req: FinishUploadRequest) -> Result<ContentId, VaultError> {
    validate_request(&req)?; // Validate input
    // rate_guard(caller())?;
    check_cycles()?;
    // Add authorization
    upload_service::finish_chunked_upload(req.upload_id, &req.sha256_checksum_hex, caller())
}

// --- Content Download Endpoint ---
#[update] // May need update if rate limiting/logging state changes
async fn request_download(req: RequestDownloadRequest) -> Result<DownloadInfo, VaultError> {
    validate_request(&req)?; // Validate input
    // rate_guard(caller())?;
    check_cycles()?;
    // Add authorization: Owner or unlocked member?
    // Add rate limit check (3 per day per PRD)

    // Placeholder implementation - requires asset canister/gateway integration
    ic_cdk::println!(
        "Download requested for vault {} content {}",
        req.vault_id,
        req.content_id
    );
    Err(VaultError::InternalError(
        "Download functionality not yet implemented".to_string(),
    ))
}

// --- Unlock Endpoint ---
#[update]
async fn trigger_unlock(req: TriggerUnlockRequest) -> Result<(), VaultError> {
    validate_request(&req)?; // Validate input
    // rate_guard(caller())?;
    check_cycles()?;
    // Add authorization: Witness or admin?
    vault_service::trigger_unlock(&req.vault_id, caller())
}

// --- Maintenance Endpoint ---

#[update(guard = "cron_or_admin_guard")] // Guarded update
async fn daily_maintenance() -> Result<(), VaultError> {
    ic_cdk::println!("Executing daily maintenance triggered by {}", caller());
    check_cycles()?;
    scheduler_service::run_daily_maintenance().await
}

// --- Admin & Metrics Endpoints (Phase 6) ---

#[query(guard = "admin_guard")] // Guarded query
async fn list_vaults(req: ListRequest) -> Result<ListVaultsResponse, VaultError> {
    let offset = req.offset.unwrap_or(0) as usize;
    let limit = req.limit.unwrap_or(10) as usize; // Default limit 10

    let vaults: Vec<VaultSummary> = VAULT_CONFIGS.with(|map_ref| {
        map_ref
            .borrow()
            .iter()
            .skip(offset)
            .take(limit)
            .map(|(_, storable_config)| {
                let config = storable_config.0; // Get inner VaultConfig
                VaultSummary {
                    vault_id: config.id.clone(),
                    owner: config.owner,
                    status: config.status.clone(), // Clone if status is complex
                    storage_used_bytes: config.storage_used_bytes,
                    plan: config.plan.clone(),
                    created_at: config.created_at,
                }
            })
            .collect()
    });

    // Get total count for pagination info
    let total = VAULT_CONFIGS.with(|map_ref| map_ref.borrow().len());

    Ok(ListVaultsResponse { vaults, total })
}

#[query(guard = "admin_guard")] // Guarded query
async fn list_billing(req: ListRequest) -> Result<ListBillingResponse, VaultError> {
    let offset = req.offset.unwrap_or(0) as u64; // Log indices are u64
    let limit = req.limit.unwrap_or(10) as usize; // Default limit 10

    let entries: Vec<BillingEntry> = BILLING_LOG.with(|log_ref| {
        let log = log_ref.borrow();
        let total_len = log.len();
        let start_index = offset.min(total_len); // Ensure offset is within bounds

        log.iter()
           .skip(start_index as usize) // Skip based on offset
           .take(limit) // Take up to the limit
           .map(|storable_entry| storable_entry.0) // Get inner BillingEntry
           .collect()
    });

    let total = BILLING_LOG.with(|log_ref| log_ref.borrow().len());

    Ok(ListBillingResponse { entries, total })
}

// --- Certified Metrics Endpoint (Task 5.3 & 6) ---
#[query] // Typically public, but could be guarded
async fn get_metrics() -> Result<GetMetricsResponse, VaultError> {
    // check_cycles()?; // Decide if queries should check cycles

    let persisted_metrics = get_stored_metrics(); // Fetch from stable storage
    let cycle_balance = Nat::from(canister_balance128());

    let response = GetMetricsResponse {
        metrics: persisted_metrics,
        cycle_balance,
        // cycles_burn_per_day: Nat::from(0u32), // Placeholder calculation TBD
    };

    // Serialize the response for certification
    match ciborium::into_writer(&response, Vec::new()) {
        Ok(bytes) => {
            set_certified_data(&bytes); // Certify the serialized data
            ic_cdk::println!("Metrics certified successfully.");
        }
        Err(e) => {
             ic_cdk::println!("Error serializing metrics for certification: {:?}", e);
             // Log the error but potentially still return the data without certification?
             // For now, return an internal error.
             return Err(VaultError::InternalError("Failed to certify metrics data".to_string()));
        }
    };

    Ok(response)
}

// --- Candid Export ---

// Helper function to generate the candid file
#[query(name = "__get_candid_interface_tmp_path")]
fn export_candid() -> String {
    ic_cdk::export_candid!();
    __export_generated_candid()
}

"" 