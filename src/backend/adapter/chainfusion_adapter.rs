use crate::error::VaultError;
use crate::models::common::{PrincipalId, Timestamp, SessionId};
use crate::models::payment::{E8s, PayMethod};
use candid::{CandidType, Nat, Principal};
use serde::{Deserialize, Serialize};
use ic_cdk::api::management_canister::http_request::{HttpMethod, CanisterHttpRequestArgument, HttpResponse, http_request, HttpHeader};
use serde_json; // Add serde_json for JSON handling

// Placeholder URL for the ChainFusion service API
// TODO: Replace with the actual ChainFusion API endpoint URL
const CHAINFUSION_API_URL: &str = "https://chainfusion.example.com/api";
// Define API paths
const INIT_SWAP_PATH: &str = "/init_swap";
const SWAP_STATUS_PATH: &str = "/swap_status";

const HTTP_OUTCALL_CYCLES: u128 = 100_000_000; // Cycles for the HTTP outcall
const MAX_RESPONSE_BYTES: u64 = 1024 * 10; // Max 10KiB response

// --- ChainFusion API Request/Response Structs (Placeholders) ---

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
pub struct ChainFusionInitRequest {
    pub session_id: SessionId,
    pub target_principal: String, // The principal (derived subaccount) to receive ICP
    pub target_amount_icp_e8s: E8s,
    // Potentially add user's desired source token if known, e.g., "ETH", "BTC"
}

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
pub struct ChainFusionInitResponse {
    pub swap_address: String, // Address for the user to send their source token (e.g., ETH address)
    pub source_token_symbol: String, // e.g., "ETH"
    pub estimated_source_amount: String, // e.g., "0.05" (as string for precision)
    pub expires_at: u64, // Timestamp when this swap offer expires
}

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
pub struct ChainFusionStatusRequest {
    pub session_id: SessionId,
}

#[derive(CandidType, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ChainFusionSwapStatus {
    Pending,      // Swap initiated, waiting for user deposit
    Processing,   // User deposit detected, swap in progress
    Completed,    // Swap successful, ICP sent to target_principal
    Expired,      // Swap offer expired before user deposit
    Failed(String), // Swap failed (e.g., insufficient funds, API error)
}

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
pub struct ChainFusionStatusResponse {
    pub session_id: SessionId,
    pub status: ChainFusionSwapStatus,
    pub icp_tx_hash: Option<String>, // ICP transaction hash if status is Completed
}

// --- ChainFusion Adapter Client Logic ---

/// Calls the ChainFusion service to initialize a swap.
/// Returns the swap address and expected token details for the user.
pub async fn initialize_chainfusion_swap(
    req: ChainFusionInitRequest,
) -> Result<ChainFusionInitResponse, VaultError> {
    ic_cdk::print(format!("🔗 INFO: Initializing ChainFusion swap for session {}", req.session_id));

    // 1. Serialize the request to JSON
    let request_body = serde_json::to_vec(&req)
        .map_err(|e| VaultError::SerializationError(format!("Failed to serialize ChainFusionInitRequest: {}", e)))?;

    // 2. Construct the HTTP request argument
    let request_arg = CanisterHttpRequestArgument {
        url: format!("{}{}", CHAINFUSION_API_URL, INIT_SWAP_PATH),
        method: HttpMethod::POST,
        body: Some(request_body),
        max_response_bytes: Some(MAX_RESPONSE_BYTES),
        transform: None, // No transform function for now
        headers: vec![
            HttpHeader { name: String::from("Content-Type"), value: String::from("application/json") },
        ],
    };

    // 3. Make the HTTP outcall
    ic_cdk::print("🔗 INFO: Making HTTP outcall to initialize ChainFusion swap...");
    match http_request(request_arg, HTTP_OUTCALL_CYCLES).await {
        Ok((response,)) => {
            ic_cdk::print(format!("🔗 INFO: Received HTTP response with status {}", response.status));
            if response.status >= 200 && response.status < 300 {
                // 4. Deserialize the response body
                serde_json::from_slice::<ChainFusionInitResponse>(&response.body)
                    .map_err(|e| VaultError::SerializationError(format!("Failed to deserialize ChainFusionInitResponse: {}", e)))
            } else {
                Err(VaultError::HttpError(format!(
                    "ChainFusion init_swap API returned error status {}: {}",
                    response.status,
                    String::from_utf8_lossy(&response.body)
                )))
            }
        }
        Err((code, msg)) => {
            ic_cdk::eprintln!("🔥 ERROR: HTTP outcall failed: {:?} - {}", code, msg);
            Err(VaultError::HttpError(format!(
                "Failed to call ChainFusion init_swap API: {:?} - {}",
                code, msg
            )))
        }
    }
}

/// Calls the ChainFusion service to check the status of a swap.
pub async fn check_chainfusion_swap_status(
    session_id: &SessionId,
) -> Result<ChainFusionStatusResponse, VaultError> {
    ic_cdk::print(format!("🔗 INFO: Checking ChainFusion swap status for session {}", session_id));
    let req = ChainFusionStatusRequest { session_id: session_id.clone() };

    // 1. Serialize the request (assuming status requires POST with body)
    let request_body = serde_json::to_vec(&req)
        .map_err(|e| VaultError::SerializationError(format!("Failed to serialize ChainFusionStatusRequest: {}", e)))?;

    // 2. Construct the HTTP request argument (Assuming POST, adjust if GET)
    let request_arg = CanisterHttpRequestArgument {
        url: format!("{}{}", CHAINFUSION_API_URL, SWAP_STATUS_PATH),
        method: HttpMethod::POST, // Or HttpMethod::GET if applicable
        body: Some(request_body), // Or None if GET with query parameters
        max_response_bytes: Some(MAX_RESPONSE_BYTES),
        transform: None,
        headers: vec![
            HttpHeader { name: String::from("Content-Type"), value: String::from("application/json") },
        ],
    };

    // 3. Make the HTTP outcall
    ic_cdk::print("🔗 INFO: Making HTTP outcall to check ChainFusion swap status...");
    match http_request(request_arg, HTTP_OUTCALL_CYCLES).await {
        Ok((response,)) => {
            ic_cdk::print(format!("🔗 INFO: Received HTTP response with status {}", response.status));
            if response.status >= 200 && response.status < 300 {
                // 4. Deserialize the response body
                serde_json::from_slice::<ChainFusionStatusResponse>(&response.body)
                    .map_err(|e| VaultError::SerializationError(format!("Failed to deserialize ChainFusionStatusResponse: {}", e)))
            } else {
                Err(VaultError::HttpError(format!(
                    "ChainFusion swap_status API returned error status {}: {}",
                    response.status,
                    String::from_utf8_lossy(&response.body)
                )))
            }
        }
        Err((code, msg)) => {
            ic_cdk::eprintln!("🔥 ERROR: HTTP outcall failed: {:?} - {}", code, msg);
            Err(VaultError::HttpError(format!(
                "Failed to call ChainFusion swap_status API: {:?} - {}",
                code, msg
            )))
        }
    }
} 