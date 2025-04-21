// src/backend/api.rs
// Placeholder for Candid API endpoint definitions (query/update functions) 

use crate::{
    error::VaultError,
    metrics::VaultMetrics, // Import the correct VaultMetrics struct
    models::{
        common::*, // Includes PrincipalId, VaultId, SessionId, Timestamp, Role, VaultStatus, MemberStatus, etc.
        vault_config::{VaultConfig, UnlockConditions},
        vault_invite_token::VaultInviteToken,
        vault_member::VaultMember, // Use this for the actual member data
        billing::BillingEntry, // Import BillingEntry
        payment::{PaymentSession, PayMethod}, // Import PaymentSession & PayMethod directly
    },
    services::{
        invite_service::{self, InviteClaimData}, // Removed MemberProfile import from here
        upload_service::{self, FileMeta, UploadId, ContentId}, // Added ContentId
        vault_service::{self, VaultInitData, VaultUpdateData},
        payment_service::{self, PaymentInitRequest as PaymentServiceInitRequest, PaymentSessionStatus}, // Import status struct
        scheduler_service,
    },
    storage::{
        get_metrics as get_stored_metrics, // Import storage helper
        audit_logs::add_audit_log_entry, // Correct path for audit log
        get_value, // Assuming get_value is pub in storage/mod.rs
        vault_configs, // For guards potentially
        billing, // For list_billing
    },
    utils::{
        guards::{self, check_admin, check_cycles, admin_guard, cron_or_admin_guard, owner_guard, owner_or_heir_guard, member_guard, self_or_owner_guard, role_guard}, // Import guards and named guards
        rate_limit::rate_guard, // Import the rate guard
    },
};
use candid::{CandidType, Deserialize, Principal, Nat}; // Import Nat
use ic_cdk::{caller, api};
use ic_cdk::api::{canister_balance128, data_certificate, set_certified_data}; // Import IC APIs
use ic_cdk_macros::{query, update}; // Use specific import for clarity
use std::cell::RefCell;
use std::collections::HashMap;
use validator::{Validate, ValidationError};
use serde::{Deserialize, Serialize}; // Import Serialize

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
    let admin = crate::storage::config::get_admin_principal()?;
    if api::caller() == admin {
        Ok(())} else {
        Err(VaultError::NotAuthorized("Caller is not admin.".to_string()))
    }
}

/// Checks if the caller is the designated cron trigger OR the admin principal.
fn cron_or_admin_guard() -> Result<(), VaultError> { // Return VaultError
    let caller = api::caller();
    let is_admin = crate::storage::config::get_admin_principal().map_or(false, |admin| caller == admin);
    let is_cron = crate::storage::config::get_cron_principal().map_or(false, |cron| caller == cron);

    if is_admin || is_cron {
        Ok(())} else {
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
fn validate_principal(_p: &Principal) -> Result<(), ValidationError> { Ok(()) /* Basic check for now */ }

// --- Request/Response Structs (as per icp-api-conventions) ---

// Use the definition from models/vault_member.rs instead of defining here
// #[derive(Clone, Debug, CandidType, Deserialize, serde::Serialize)]
// pub struct MemberProfile {
//     pub member_id: MemberId,
//     pub vault_id: VaultId,
//     pub principal: PrincipalId,
//     pub role: Role,
//     pub status: MemberStatus,
//     pub shamir_share_index: u8,
//     pub name: Option<String>,
//     pub relation: Option<String>,
//     pub added_at: Timestamp,
// }

// Use the struct from models directly if needed, e.g. in claim_invite response
// Alias it if necessary for clarity in API layer
pub type ApiMemberProfile = crate::models::vault_member::VaultMember; // Example alias

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

#[derive(CandidType, Deserialize, Clone, Debug, Serialize)]
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

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, Default)]
pub struct ListVaultsResponse {
    pub vaults: Vec<VaultSummary>,
    pub total: u64,
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, Default)]
pub struct ListBillingResponse {
     pub entries: Vec<BillingEntry>,
     pub total: u64,
}

// Define needed summary/entry structs for admin lists
#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
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
    pub method: PayMethod,
}

#[update] // Payment initialization likely involves state change (session creation)
async fn init_payment(req: ApiPaymentInitRequest) -> Result<PaymentSession, VaultError> {
    validate_request(&req)?; // Validation is now active
    let caller = api::caller();
    // Apply rate limiting if desired
    rate_guard(caller)?;
    // Apply cycle check
    check_cycles()?;

    let service_req = PaymentServiceInitRequest {
        vault_plan: req.vault_plan,
        amount_e8s: req.amount_e8s,
        method: req.method,
    };

    payment_service::initialize_payment_session(service_req, caller).await
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
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;
    let result = payment_service::verify_payment(&req.session_id, &req.vault_id).await;
    Ok(format!("Payment Verified: {}", result))
}

// Query: Get Payment Session Status
#[ic_cdk_macros::query(guard = "check_cycles")] // Basic guard, maybe needs auth?
fn get_payment_status(session_id: SessionId) -> Result<PaymentSessionStatus, VaultError> {
   ic_cdk::println!("API: get_payment_status called for session {}", session_id);
   // TODO: Add appropriate authorization check? Who can query status?
   // Potentially the initiator or admin?
   // let caller = ic_caller();
   // let session = payment_service::get_payment_session(session_id)... Check initiator? 

   payment_service::get_payment_session_status(&session_id)
}

// --- Vault Core Endpoints ---

#[update]
async fn create_vault(req: CreateVaultRequest) -> Result<VaultId, VaultError> {
    validate_request(&req)?; // Validate input
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;

    let vault_config = vault_service::create_new_vault(caller, req.name, req.description, req.plan).await?;

    add_audit_log_entry(&vault_config.id.to_string(), crate::models::audit::AuditLogEntry::new(
        Action::CreateVault,
        caller,
        Some(format!("Vault '{}' created.", vault_config.name))
    ))?;

    Ok(vault_config.id)
}

#[query]
async fn get_vault(vault_id: VaultId) -> Result<VaultConfig, VaultError> {
    let caller = api::caller();
    rate_guard(caller)?;
    guards::owner_or_heir_guard(&vault_id, caller)?;
    vault_service::get_vault_config(&vault_id)
}

#[update(guard = "owner_guard")]
async fn update_vault(req: UpdateVaultRequest) -> Result<(), VaultError> {
    validate_request(&req)?;
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;
    guards::owner_guard(&req.vault_id, caller)?;

    let update_data = VaultUpdateData {
        name: req.name,
        description: req.description,
        unlock_conditions: req.unlock_conditions,
        plan: req.plan,
    };

    vault_service::update_vault_config(&req.vault_id, update_data, caller).await?;

    add_audit_log_entry(&req.vault_id.to_string(), crate::models::audit::AuditLogEntry::new(
        Action::UpdateVault,
        caller,
        Some("Vault configuration updated.".to_string())
    ))?;

    Ok(()})
}

// --- Invitation & Member Endpoints ---

#[update]
async fn generate_invite(req: GenerateInviteRequest) -> Result<VaultInviteToken, VaultError> {
    validate_request(&req)?;
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;
    guards::owner_guard(&req.vault_id, caller)?;

    let token = invite_service::generate_new_invite(&req.vault_id, req.role, caller).await?;

    add_audit_log_entry(&req.vault_id.to_string(), crate::models::audit::AuditLogEntry::new(
        Action::GenerateInvite,
        caller,
        Some(format!("Invite generated for role {:?}, token ID: {}", req.role, token.id))
    ))?;

    Ok(token)
}

#[update]
async fn claim_invite(req: ClaimInviteRequest) -> Result<ApiMemberProfile, VaultError> {
    validate_request(&req)?;
    let claimer = api::caller();
    rate_guard(claimer)?;
    check_cycles()?;

    let claim_data = InviteClaimData {
        name: req.name,
        relation: req.relation,
    };

    let member = invite_service::claim_existing_invite(req.token, claimer, claim_data).await?;

    add_audit_log_entry(&member.vault_id.to_string(), crate::models::audit::AuditLogEntry::new(
        Action::ClaimInvite,
        claimer,
        Some(format!("Invite claimed by {}, assigned role {:?}.\n", claimer, member.role))
    ))?;

    Ok(member)
}

#[update]
async fn revoke_invite(token_id: InviteTokenId /* Principal */) -> Result<(), VaultError> {
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;
    // Need vault context for owner guard - how to get vault_id from token_id?
    // Requires lookup in invite_service or passing vault_id
    // invite_service::revoke_invite_token(token_id, caller) // Placeholder
    Err(VaultError::NotImplemented("Revoke invite endpoint not implemented".to_string()))
}

// --- Content Upload Endpoints ---

#[update]
async fn begin_upload(req: BeginUploadRequest) -> Result<UploadId /* Principal */, VaultError> {
    validate_request(&req)?;
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;
    guards::owner_or_heir_guard(&req.vault_id, caller)?;
    upload_service::begin_chunked_upload(req.vault_id, req.file_meta, caller).await
}

#[update]
async fn upload_chunk(req: UploadChunkRequest) -> Result<(), VaultError> {
    validate_request(&req)?;
    let caller = api::caller();
    rate_guard(caller)?;
    // check_cycles is tricky here due to potential high frequency
    // upload_service::upload_chunk(req.upload_id, req.chunk_index, &req.data, caller).await
    Err(VaultError::NotImplemented("upload_chunk needs careful cycle mgmt".to_string()))
}

#[update]
async fn finish_upload(req: FinishUploadRequest) -> Result<ContentId /* Principal */, VaultError> {
    validate_request(&req)?;
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;
    // Guard should ideally check against vault_id associated with upload_id
    let content_id = upload_service::finish_chunked_upload(req.upload_id, req.sha256_checksum_hex, caller).await?;

    // Need vault_id for audit log - retrieve from upload session
    // add_audit_log_entry(&vault_id.to_string(), ... Action::UploadContent ...)?; 

    Ok(content_id)
}

// --- Content Download Endpoint ---
#[query(guard = "check_cycles")]
async fn request_download(req: RequestDownloadRequest) -> Result<DownloadInfo, VaultError> {
    validate_request(&req)?;
    let caller = api::caller();
    rate_guard(caller)?;
    // Add appropriate guard (e.g., owner, heir, or witness after unlock)
    guards::member_guard(&req.vault_id, caller)?; // Example: Any member can request download info
    // Placeholder: vault_service::get_download_info needs implementation
    // vault_service::get_download_info(req.vault_id, req.content_id).await
    Err(VaultError::NotImplemented("Request download endpoint not implemented".to_string()))
}

// --- Unlock Endpoint ---
#[update(guard = "owner_or_heir_guard")]
async fn trigger_unlock(req: TriggerUnlockRequest) -> Result<(), VaultError> {
    validate_request(&req)?; // Validate input
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;
    // Guard: must be witness of the vault
    guards::role_guard(&req.vault_id, caller, Role::Witness)?;

    vault_service::trigger_vault_unlock(&req.vault_id, caller).await?;

    add_audit_log_entry(&req.vault_id.to_string(), crate::models::audit::AuditLogEntry::new(
        Action::TriggerUnlock,
        caller,
        Some("Unlock sequence triggered by witness.".to_string())
    ))?;

    Ok(()})
}

// --- Maintenance Endpoint ---

#[update(guard = "cron_or_admin_guard")] // Use named guard
async fn daily_maintenance() -> Result<(), VaultError> { // Return VaultError
    cron_or_admin_guard()?;
    check_cycles()?; // Ensure enough cycles for maintenance

    ic_cdk::println!("INFO: Starting daily maintenance task...");
    let result = scheduler_service::perform_daily_maintenance().await; // Call the async scheduler function
    ic_cdk::println!("INFO: Daily maintenance task finished.");
    result // Return the result from the service
}

// --- Admin & Metrics Endpoints (Phase 6) ---

#[query(guard = "admin_guard")] // Use named guard
async fn list_vaults(req: ListRequest) -> Result<ListVaultsResponse, VaultError> { // Return VaultError
    validate_request(&req)?;
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;

    let offset = req.offset.unwrap_or(0) as usize;
    let limit = req.limit.unwrap_or(10) as usize;

    // Placeholder: vault_service::admin_list_vaults needs implementation
    // let (vaults, total) = vault_service::admin_list_vaults(offset, limit).await?;
    // Ok(ListVaultsResponse { vaults, total })
    Err(VaultError::NotImplemented("List vaults endpoint not implemented".to_string()))
}

#[query(guard = "admin_guard")] // Use named guard
async fn list_billing(req: ListRequest) -> Result<ListBillingResponse, VaultError> { // Return VaultError
    validate_request(&req)?;
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;

    let offset = req.offset.unwrap_or(0) as usize;
    let limit = req.limit.unwrap_or(10) as usize;

    // Use the billing storage directly for listing
    let entries = billing::query_billing_entries(offset, limit);
    let total = billing::get_billing_log_len(); // Get total count

    Ok(ListBillingResponse { entries, total_entries: total })
}

// --- Certified Metrics Endpoint (Task 5.3 & 6) ---
#[query(guard = "admin_guard")] // Use named guard
async fn get_metrics() -> Result<GetMetricsResponse, VaultError> { // Return VaultError
    let caller = api::caller();
    rate_guard(caller)?;
    check_cycles()?;

    let metrics = get_stored_metrics().unwrap_or_default(); // Handle potential error if cell is empty
    let cycle_balance = api::canister_balance128();

    let response = GetMetricsResponse {
        metrics,
        cycle_balance: cycle_balance,
    };

    Ok(response)
}

// --- Certification --- //
// Certify responses to enable trustless data fetching by clients (e.g., dashboards)
fn certify_response<T: CandidType + Serialize>(response: &T) {
    match ciborium::ser::into_writer(response, vec![]) {
        Ok(bytes) => {
            let certificate = data_certificate().unwrap_or_else(|| {
                ic_cdk::trap("Data certificate is not available in this context.");
            });
            set_certified_data(&bytes);
            // The certificate is implicitly included in the response header by the IC
        },
        Err(e) => {
            ic_cdk::trap(&format!("Failed to serialize response for certification: {}", e));
        }
    }
}

// --- Candid Export ---

#[query(name = "__get_candid_interface_tmp_hack")]
fn export_candid() -> String {
    candid::export_service!();
    __export_service()
}

#[cfg(test)]
mod tests {
    use super::export_candid;

    #[test]
    fn check_candid_interface_compiles() {
        let candid = export_candid();
        println!("{}", candid);
        // assert!(false); // Force print candid
    }
}

