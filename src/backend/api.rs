// src/backend/api.rs
// Placeholder for Candid API endpoint definitions (query/update functions) 

use crate::{
    error::VaultError,
    metrics::VaultMetrics, // Import the correct VaultMetrics struct
    models::common::*,
    models::vault_config::{VaultConfig, UnlockConditions},
    models::vault_invite_token::VaultInviteToken,
    models::vault_member::VaultMember,
    services::{
        invite_service::{self, InviteClaimData, MemberProfile},
        upload_service::{self, FileMeta, UploadId},
        vault_service::{self, VaultInitData, VaultUpdateData},
        payment_service::{self, PaymentInitRequest as PaymentServiceInitRequest, PayMethod, SessionId},
        scheduler_service,
    },
    storage::get_metrics as get_stored_metrics, // Import storage helper
    utils::guards::{check_admin, check_cycles, admin_guard, cron_or_admin_guard, owner_guard, owner_or_heir_guard, member_or_heir_guard, self_or_owner_guard}, // Import guards and named guards
    utils::rate_limit::rate_guard, // Import the rate guard
    models::billing::BillingEntry, // Import BillingEntry
    storage::{VAULT_CONFIGS, BILLING_LOG, Cbor}, // Import storage structures
};
use candid::{CandidType, Deserialize, Principal, Nat}; // Import Nat
use ic_cdk::{caller, api};
use ic_cdk::api::{canister_balance128, data_certificate, set_certified_data}; // Import IC APIs
use ic_cdk_macros::{query, update}; // Use specific import for clarity
use crate::models::payment::PaymentSession; // Import PaymentSession for return type
use std::cell::RefCell;
use std::collections::HashMap;
use validator::{Validate, ValidationError};
use serde::Deserialize; // Import Deserialize

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
    req.validate().map_err(|e| VaultError::InvalidInput(e.to_string()))
}

// Add Principal validator if complex validation needed
fn validate_principal(p: &Principal) -> Result<(), ValidationError> { Ok(()) /* Basic check */ }

// --- Request/Response Structs (as per icp-api-conventions) ---

// Placeholder for profile data returned after claiming invite
// Moved the definition from API or ensure it's consistent if defined in models
#[derive(Clone, Debug, CandidType, Deserialize, serde::Serialize)]
pub struct MemberProfile {
    pub member_id: MemberId,
    pub vault_id: VaultId,
    pub principal: PrincipalId,
    pub role: Role,
    pub status: MemberStatus,
    pub shamir_share_index: u8,
    pub name: Option<String>,
    pub relation: Option<String>,
    pub added_at: Timestamp,
}


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
    #[validate(custom = "validate_principal")]
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
    #[validate(custom = "validate_principal")]
    pub vault_id: VaultId,
    pub role: Role,
}

// Claim Invite
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct ClaimInviteRequest {
    #[validate(custom = "validate_principal")]
    pub token: InviteTokenId,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub relation: Option<String>,
}

// Upload
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct BeginUploadRequest {
    #[validate(custom = "validate_principal")]
    pub vault_id: VaultId,
    #[validate]
    pub file_meta: FileMeta,
}

#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct UploadChunkRequest {
    #[validate(custom = "validate_principal")]
    pub upload_id: UploadId,
    pub chunk_index: u32,
    #[serde(with = "serde_bytes")]
    #[validate(length(min = 1, max = 524288))]
    pub data: Vec<u8>,
}

#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct FinishUploadRequest {
    #[validate(custom = "validate_principal")]
    pub upload_id: UploadId,
    #[validate(length(min = 64, max = 64))]
    pub sha256_checksum_hex: String,
}

// Download
#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct RequestDownloadRequest {
    #[validate(custom = "validate_principal")]
    pub vault_id: VaultId,
    #[validate(custom = "validate_principal")]
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
    #[validate(custom = "validate_principal")]
    pub vault_id: VaultId,
    // Add any necessary witness data/proof if needed
}

// Admin & Listing
#[derive(CandidType, Deserialize, Validate)]
pub struct ListRequest {
    #[validate(range(min = 0))]
    pub offset: Option<u32>,
    #[validate(range(min = 1, max = 100))]
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
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct VaultSummary {
     pub vault_id: VaultId,
     pub owner: PrincipalId,
     pub status: VaultStatus,
     pub storage_used_bytes: u64,
     pub plan: String,
     pub created_at: Timestamp,
}

// Define the response type for get_metrics including dynamic cycle balance
#[derive(CandidType, Deserialize, Serialize, Clone, Debug, Default)]
pub struct GetMetricsResponse {
    pub metrics: VaultMetrics,
    pub cycle_balance: u128,
    // pub cycles_burn_per_day: Nat, // Calculation TBD
}

// --- Candid Endpoints ---

// --- Payment Endpoints (Phase 4) ---

#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct ApiPaymentInitRequest { // Renamed to avoid conflict if needed
    #[validate(length(min = 1))]
    pub vault_plan: String,
    #[validate(range(min = 1))]
    pub amount_e8s: u64, // E8s
    pub method: crate::models::payment::PayMethod,
}

#[update] // Payment initialization likely involves state change (session creation)
async fn init_payment(req: ApiPaymentInitRequest) -> Result<PaymentSession, VaultError> {
    validate_request(&req)?; // Validation is now active
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

#[derive(CandidType, Deserialize, Clone, Debug, Validate)]
pub struct VerifyPaymentRequest {
    #[validate(length(min = 1))]
    pub session_id: SessionId,
    #[validate(custom = "validate_principal")]
    pub vault_id: VaultId,
}

#[update] // Verification likely updates vault/session state
async fn verify_payment(req: VerifyPaymentRequest) -> Result<String, VaultError> {
    validate_request(&req)?;
    check_cycles()?;
    payment_service::verify_and_associate_payment(req.session_id, req.vault_id).await
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
    vault_service::create_new_vault(init_data).await
}

#[query]
async fn get_vault(vault_id: VaultId) -> Result<VaultConfig, VaultError> {
    // validate_request(&vault_id)? // Cannot validate String directly, use length if needed
    vault_service::get_vault_config(&vault_id).await
}

#[update(guard = "owner_guard")]
async fn update_vault(req: UpdateVaultRequest) -> Result<(), VaultError> {
    validate_request(&req)?;
    // rate_guard(caller())?;
    check_cycles()?;

    let update_data = vault_service::VaultUpdateData {
        name: req.name,
        description: req.description,
        unlock_conditions: req.unlock_conditions,
        plan: req.plan,
    };

    vault_service::update_vault_config(&req.vault_id, update_data, caller()).await
}

// --- Invitation & Member Endpoints ---

#[update]
async fn generate_invite(req: GenerateInviteRequest) -> Result<VaultInviteToken, VaultError> {
    validate_request(&req)?;
    // rate_guard(caller())?;
    check_cycles()?;

    invite_service::generate_new_invite(&req.vault_id, req.role, caller()).await
}

#[update]
async fn claim_invite(req: ClaimInviteRequest) -> Result<MemberProfile, VaultError> {
    validate_request(&req)?;
    check_cycles()?;
    let claimer = caller();
    // Map request fields to the InviteClaimData struct expected by the service
    let claim_data = invite_service::InviteClaimData {
        name: req.name,
        relation: req.relation,
    };
    // Pass the Principal token_id and mapped data
    services::invite_service::claim_existing_invite(req.token, claimer, claim_data).await
}

#[update]
async fn revoke_invite(token_id: InviteTokenId /* Principal */) -> Result<(), VaultError> {
    // Basic validation for Principal ID if needed
    // validate_principal(&token_id)?;
    check_cycles()?;
    let caller = caller();
    // Note: Service function revoke_invite_token requires owner check. 
    // This might need refactoring in the service or require vault_id here.
    // For now, calling the service directly.
    services::invite_service::revoke_invite_token(token_id, caller)
}

// --- Content Upload Endpoints ---

#[update]
async fn begin_upload(req: BeginUploadRequest) -> Result<UploadId /* Principal */, VaultError> {
    validate_request(&req)?;
    let caller = caller();
    owner_guard(&req.vault_id)?; // Assuming owner_guard accepts Principal ref
    check_cycles()?;
    services::upload_service::begin_chunked_upload(req.vault_id, req.file_meta, caller).await
}

#[update]
async fn upload_chunk(req: UploadChunkRequest) -> Result<(), VaultError> {
    validate_request(&req)?;
    check_cycles()?;
    let caller = caller();
    services::upload_service::upload_chunk(req.upload_id, req.chunk_index, &req.data, caller).await
}

#[update]
async fn finish_upload(req: FinishUploadRequest) -> Result<ContentId /* Principal */, VaultError> {
    validate_request(&req)?;
    check_cycles()?;
    services::upload_service::finish_chunked_upload(req.upload_id, req.sha256_checksum_hex).await
}

// --- Content Download Endpoint ---
#[query(guard = "check_cycles")]
async fn request_download(req: RequestDownloadRequest) -> Result<DownloadInfo, VaultError> {
    validate_request(&req)?;
    member_or_heir_guard(&req.vault_id)?; // Assuming guard accepts Principal ref
    services::vault_service::get_download_info(req.vault_id, req.content_id).await
}

// --- Unlock Endpoint ---
#[update(guard = "owner_or_heir_guard")]
async fn trigger_unlock(req: TriggerUnlockRequest) -> Result<(), VaultError> {
    validate_request(&req)?; // Validate input
    // rate_guard(caller())?;
    check_cycles()?;

    // Only a witness should trigger this? Or admin?
    // Add guard or check caller role
    vault_service::trigger_unlock(&req.vault_id, caller()).await
}

// --- Maintenance Endpoint ---

#[update(guard = "cron_or_admin_guard")] // Use named guard
async fn daily_maintenance() -> Result<(), VaultError> { // Return VaultError
    check_cycles()?; // Keep internal cycle check
    scheduler::perform_daily_maintenance().await // Call the async scheduler function
}

// --- Admin & Metrics Endpoints (Phase 6) ---

#[query(guard = "admin_guard")] // Use named guard
async fn list_vaults(req: ListRequest) -> Result<ListVaultsResponse, VaultError> { // Return VaultError
    validate_request(&req)?;
    check_cycles()?; // Keep internal cycle check

    let offset = req.offset.unwrap_or(0) as u64;
    let limit = req.limit.unwrap_or(10) as usize;

    // Fetch vault data
    let (vaults, total) = vault_service::list_all_vaults(offset, limit).await?;

    // Convert full VaultConfig to VaultSummary
    let summaries = vaults.into_iter().map(|config| VaultSummary {
        vault_id: config.vault_id.clone(), // Corrected field name
        owner: config.owner,
        name: config.name,
        status: config.status,
        created_at: config.created_at,
        expires_at: config.expires_at,
    }).collect();

    let response = ListVaultsResponse { vaults: summaries, total_vaults: total };
    certify_response(&response); // Certify if needed
    Ok(response)
}

#[query(guard = "admin_guard")] // Use named guard
async fn list_billing(req: ListRequest) -> Result<ListBillingResponse, VaultError> { // Return VaultError
    validate_request(&req)?;
    check_cycles()?; // Keep internal cycle check

    let offset = req.offset.unwrap_or(0) as u64;
    let limit = req.limit.unwrap_or(10) as usize;

    // Fetch billing data
    let (entries, total) = payment_service::list_billing_entries(offset, limit).await?;

    let response = ListBillingResponse { entries, total_entries: total };
    certify_response(&response); // Certify if needed
    Ok(response)
}

// --- Certified Metrics Endpoint (Task 5.3 & 6) ---
#[query(guard = "admin_guard")] // Use named guard
async fn get_metrics() -> Result<GetMetricsResponse, VaultError> { // Return VaultError
    // Fetch aggregated metrics from storage or calculate them
    let metrics = crate::metrics::get_vault_metrics().await?;

    // Get current cycle balance
    let cycle_balance = ic_cdk::api::canister_balance128();

    let response = GetMetricsResponse {
        metrics,
        cycle_balance: cycle_balance,
    };
    certify_response(&response); // Certify if needed
    Ok(response)
}

// --- Certification --- //
// Certify responses to enable trustless data fetching by clients (e.g., dashboards)
fn certify_response<T: CandidType + Serialize>(response: &T) {
    // Correctly serialize to Vec<u8>
    let mut writer = Vec::new();
    match ciborium::into_writer(&response, &mut writer) {
        Ok(_) => {
            set_certified_data(&writer); // Certify the serialized data
        },
        Err(e) => {
            ic_cdk::eprintln!("🔥 API ERROR: Failed to serialize response for certification: {:?}", e);
            // Optionally trap or handle differently
        }
    }
}

// --- Candid Export ---

// Helper function (if not using the macro directly)
fn export_candid() -> String {
    use std::env;
    use std::fs::write;
    use std::path::PathBuf;

    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let did_path = dir.join("backend.did");
    // Assuming the macro generates the string, replace this line:
    let generated_did_string = "service: {} // Placeholder - macro should generate this";

    write(&did_path, generated_did_string).expect("Write failed.");
    format!("Generated Candid file at {}", did_path.display())
}

// Call the macro to generate the Candid interface
ic_cdk_macros::export_candid!();

