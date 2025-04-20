// src/backend/api.rs
// Placeholder for Candid API endpoint definitions (query/update functions) 

use crate::{
    error::VaultError,
    models::common::*,
    models::{VaultConfig, VaultInviteToken, VaultMember, UnlockConditions},
    services::{
        invite_service::{self, InviteClaimData, MemberProfile},
        upload_service::{self, FileMeta},
        vault_service::{self, VaultInitData, VaultUpdateData},
        payment_service::{self, PaymentInitRequest as PaymentServiceInitRequest},
        scheduler_service,
    },
    utils::rate_limit::rate_guard, // Import the rate guard
};
use candid::{CandidType, Deserialize, Principal};
use ic_cdk::{caller, query, update, api};
use ic_cdk_macros::query; // Use specific import for clarity
use crate::models::payment::PaymentSession; // Import PaymentSession for return type
use std::cell::RefCell;
use std::collections::HashMap;

// --- Admin Principal (Example - Load from stable memory/config later) ---
thread_local! {
    // TODO: Replace with a secure way to load/manage admin principals
    static ADMIN_PRINCIPAL: RefCell<Principal> = RefCell::new(Principal::anonymous()); // Use anonymous as safer placeholder
    // TODO: Replace with a secure way to load/manage the cron caller principal
    static CRON_CALLER_PRINCIPAL: RefCell<Principal> = RefCell::new(Principal::anonymous()); // Use anonymous as safer placeholder
}

// --- Guard Functions ---

/// Checks if the caller is the designated admin principal.
fn is_admin() -> Result<(), String> {
    let caller = api::caller();
    ADMIN_PRINCIPAL.with(|admin_ref| {
        if *admin_ref.borrow() == caller {
            Ok(())
        } else {
            Err("Caller is not authorized as an admin.".to_string())
        }
    })
}

/// Checks if the caller is the designated cron trigger OR the admin principal.
fn is_cron_caller_or_admin() -> Result<(), String> {
    let caller = api::caller();
    let is_admin_res = ADMIN_PRINCIPAL.with(|admin_ref| *admin_ref.borrow() == caller);
    let is_cron_res = CRON_CALLER_PRINCIPAL.with(|cron_ref| *cron_ref.borrow() == caller);

    if is_admin_res || is_cron_res {
        Ok(())
    } else {
        Err("Caller is not the authorized cron trigger or an admin.".to_string())
    }
}

// --- Request/Response Structs (as per icp-api-conventions) ---

// Vault Creation
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct CreateVaultRequest {
    // owner is derived from caller
    pub name: String,
    pub description: Option<String>,
    pub plan: String,
    // unlock_conditions: Option<UnlockConditions>, // Set via update?
}

// Vault Update
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct UpdateVaultRequest {
    pub vault_id: VaultId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub unlock_conditions: Option<UnlockConditions>,
    pub plan: Option<String>,
}

// Generate Invite
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct GenerateInviteRequest {
    pub vault_id: VaultId,
    pub role: Role,
}

// Claim Invite
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ClaimInviteRequest {
    pub token: InviteTokenId,
    pub name: Option<String>,
    pub relation: Option<String>,
}

// Upload
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct BeginUploadRequest {
    pub vault_id: VaultId,
    pub file_meta: FileMeta,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct UploadChunkRequest {
    pub upload_id: UploadId,
    pub chunk_index: u32,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct FinishUploadRequest {
    pub upload_id: UploadId,
    pub sha256_checksum_hex: String,
}

// Download
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct RequestDownloadRequest {
    pub vault_id: VaultId,
    pub content_id: ContentId,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct DownloadInfo {
    pub url: String, // Temporary URL (e.g., presigned)
    pub expires_at: Timestamp,
}

// Unlock
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TriggerUnlockRequest {
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

// Define Metrics struct
#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct VaultMetrics {
    // Define fields based on architecture doc
    pub total_vaults: u32,
    pub active_vaults: u32,
    pub unlocked_vaults: u32,
    pub need_setup_vaults: u32,
    pub expired_vaults: u32,
    pub storage_used_bytes: u64,
    pub cycle_balance_t: u64,
    pub cycles_burn_per_day: u64,
    pub invites_today: u32,
    pub invites_claimed: u32,
    pub unlock_avg_months: f64,
    pub scheduler_last_run: Timestamp,
}

// --- Candid Endpoints ---

// --- Payment Endpoints (Phase 4) ---

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ApiPaymentInitRequest {
    pub vault_plan: String,
    pub amount_e8s: u64, // E8s
    pub method: crate::models::payment::PayMethod,
}

#[update(guard = "rate_guard")]
async fn init_payment(req: ApiPaymentInitRequest) -> Result<PaymentSession, VaultError> {
    let caller = api::caller();
    let payment_req = PaymentServiceInitRequest {
        vault_plan: req.vault_plan,
        amount_e8s: req.amount_e8s,
        method: req.method,
    };
    payment_service::initialize_payment_session(payment_req, caller)
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct VerifyPaymentRequest {
    pub session_id: SessionId,
    pub vault_id: VaultId,
}

#[update(guard = "rate_guard")]
async fn verify_payment(req: VerifyPaymentRequest) -> Result<String, VaultError> {
    payment_service::verify_icp_payment(&req.session_id, &req.vault_id).await
}

// --- Vault Endpoints ---

#[update(guard = "rate_guard")]
async fn create_vault(req: CreateVaultRequest) -> Result<VaultId, VaultError> {
    let caller = api::caller();
    let init_data = VaultInitData {
        name: req.name,
        description: req.description,
        plan: req.plan,
        owner: caller,
    };
    vault_service::create_new_vault(init_data)
}

#[query(guard = "rate_guard")]
async fn get_vault(vault_id: VaultId) -> Result<VaultConfig, VaultError> {
    let caller = api::caller();
    let vault_config = vault_service::get_vault_config(&vault_id)?;
    let members = invite_service::list_vault_members(&vault_id, caller).unwrap_or_default(); // Allow if owner check passes
    let is_owner = vault_config.owner == caller;
    let is_member = members.iter().any(|m| m.principal == caller);

    if !is_owner && !is_member {
        ic_cdk::eprintln!("Unauthorized attempt to get vault {} by {}", vault_id, caller);
        return Err(VaultError::NotAuthorized("Caller must be the owner or a member to view vault details.".to_string()));
    }
    Ok(vault_config)
}

#[update(guard = "rate_guard")]
async fn update_vault(req: UpdateVaultRequest) -> Result<(), VaultError> {
    let caller = api::caller();
    let update_data = VaultUpdateData {
        name: req.name,
        description: req.description,
        unlock_conditions: req.unlock_conditions,
        plan: req.plan,
    };
    vault_service::update_existing_vault(&req.vault_id, update_data, caller)
}

// --- Invite Endpoints ---

#[update(guard = "rate_guard")]
async fn generate_invite(req: GenerateInviteRequest) -> Result<VaultInviteToken, VaultError> {
    let caller = api::caller();
    invite_service::generate_new_invite(&req.vault_id, req.role, caller).await
}

#[update(guard = "rate_guard")]
async fn claim_invite(req: ClaimInviteRequest) -> Result<MemberProfile, VaultError> {
    let caller = api::caller();
    let claim_data = InviteClaimData {
        name: req.name,
        relation: req.relation,
    };
    invite_service::claim_existing_invite(&req.token, caller, claim_data).await
}

// --- Upload & Download Endpoints ---

#[update(guard = "rate_guard")]
async fn begin_upload(req: BeginUploadRequest) -> Result<UploadId, VaultError> {
    let caller = api::caller();
    upload_service::begin_chunked_upload(req.vault_id, req.file_meta, caller)
}

#[update(guard = "rate_guard")]
async fn upload_chunk(req: UploadChunkRequest) -> Result<(), VaultError> {
    upload_service::upload_next_chunk(&req.upload_id, req.chunk_index, req.data)
}

#[update(guard = "rate_guard")]
async fn finish_upload(req: FinishUploadRequest) -> Result<ContentId, VaultError> {
    upload_service::finish_chunked_upload(&req.upload_id, req.sha256_checksum_hex)
}

#[update(guard = "rate_guard")] // Marked update due to potential state change (quota check)
async fn request_download(req: RequestDownloadRequest) -> Result<DownloadInfo, VaultError> {
    let caller = api::caller();
    ic_cdk::print(format!("Download requested for item {} in vault {} by {}", req.content_id, req.vault_id, caller));
    // TODO: Implement download_service::request_download_link function
    // This service function needs to:
    // 1. Check vault status (must be Unlockable)
    // 2. Check caller is an authorized member (Heir)
    // 3. Check daily download quota for the member & Increment counter (state change!)
    // 4. Generate a temporary URL
    Err(VaultError::NotImplemented("request_download".to_string())) // Placeholder
}

// --- Unlock Endpoint ---

#[update(guard = "rate_guard")] // TODO: Define appropriate guard (e.g., is_witness)
async fn trigger_unlock(req: TriggerUnlockRequest) -> Result<(), VaultError> {
    let caller = api::caller();
    ic_cdk::print(format!("Unlock triggered for vault {} by {}", req.vault_id, caller));
    // TODO: Implement vault_service::trigger_unlock_attempt function
    // This service function needs to:
    // 1. Verify caller is a Witness (or maybe owner in some cases?)
    // 2. Check vault status allows unlock trigger
    // 3. Potentially record witness trigger/approval
    // 4. Evaluate unlock conditions (quorum, time)
    // 5. If conditions met, call set_vault_status(Unlockable)
    Err(VaultError::NotImplemented("trigger_unlock".to_string())) // Placeholder
}

// --- Maintenance Endpoint ---

#[update(guard = "is_cron_caller_or_admin")]
async fn daily_maintenance() -> Result<(), VaultError> {
     ic_cdk::print("Maintenance task triggered by authorized caller.");
     scheduler_service::perform_daily_maintenance()
}

// --- Admin Endpoints ---

#[query(guard = "is_admin")]
async fn list_vaults(req: ListRequest) -> Result<ListVaultsResponse, VaultError> {
     ic_cdk::print(format!("Admin request: list_vaults ({:?})", req));
     // TODO: Implement admin_service::list_vaults(offset, limit)
     Err(VaultError::NotImplemented("list_vaults".to_string())) // Placeholder
}

#[query(guard = "is_admin")]
async fn list_billing(req: ListRequest) -> Result<ListBillingResponse, VaultError> {
    ic_cdk::print(format!("Admin request: list_billing ({:?})", req));
    // TODO: Implement admin_service::list_billing(offset, limit)
     Err(VaultError::NotImplemented("list_billing".to_string())) // Placeholder
}

#[query(guard = "is_admin")]
async fn get_metrics() -> Result<VaultMetrics, VaultError> {
     ic_cdk::print("Admin request: get_metrics");
     // TODO: Implement admin_service::get_metrics()
     Err(VaultError::NotImplemented("get_metrics".to_string())) // Placeholder
}

// --- Candid Export ---
candid::export_service!();

#[query(name = "__get_candid_interface_tmp_hack")]
fn export_candid() -> String {
    __export_service()
}

"" 