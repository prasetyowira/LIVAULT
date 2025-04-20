// src/backend/lib.rs

pub mod api;
pub mod error;
pub mod models;
pub mod services;
pub mod storage;
pub mod utils;

#[ic_cdk::init]
fn init() {
    ic_cdk::println!("LiVault backend canister initialized.");
    // Initialization logic will go here, e.g., setting up stable memory
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    ic_cdk::println!("LiVault backend canister upgraded.");
    // Post-upgrade logic, e.g., migrating stable memory
}

// Basic query endpoint for testing
#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

// Export Candid interface
ic_cdk::export_candid!(); 