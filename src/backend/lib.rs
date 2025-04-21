// src/backend/lib.rs
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use getrandom::register_custom_getrandom;
use std::cell::RefCell;
use std::time::Duration;
use ic_cdk_timers::set_timer;
use ic_cdk::api::management_canister::main::raw_rand;
use candid::Principal;
use crate::models::init::InitArgs;
use crate::storage::config as storage_config;

pub mod api;
pub mod error;
pub mod models;
pub mod services;
pub mod storage;
pub mod utils;
pub mod adapter;
pub mod metrics;

thread_local! {
    static RNG: RefCell<Option<StdRng>> = RefCell::new(None);
}

fn _restart_rng() {
    // need to reset the RNG each time the canister is restarted
    let _timer_id = ic_cdk_timers::set_timer(Duration::ZERO, || ic_cdk::spawn(async {
        let (seed,): ([u8; 32],) = ic_cdk::call(Principal::management_canister(), "raw_rand", ()).await.unwrap();
        ic_cdk::println!("Got seed");
        RNG.with(|rng| *rng.borrow_mut() = Some(StdRng::from_seed(seed)));
    }));
    ic_cdk::println!("registered timer {:?}", _timer_id);
}

#[ic_cdk::init]
fn init(args: InitArgs) {
    // Setup panic hook for better error reporting
    std::panic::set_hook(Box::new(|info| {
        ic_cdk::println!("Canister panicked: {:?}", info);
    }));
    // Initialize configuration from arguments
    storage_config::init_config(
        args.admin_principal,
        args.cron_principal,
        args.min_cycles_threshold
    );

    _restart_rng();

    ic_cdk::println!("LiVault backend canister initialized.");
    // Initialization logic will go here, e.g., setting up stable memory
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    _restart_rng();
    ic_cdk::println!("LiVault backend canister upgraded.");
    // Post-upgrade logic, e.g., migrating stable memory
}

fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    ic_cdk::println!("custom_getrandom");
    RNG.with(|rng| rng.borrow_mut().as_mut().unwrap().fill_bytes(buf));
    Ok(())
}
register_custom_getrandom!(custom_getrandom);

// Basic query endpoint for testing
#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

// Export Candid interface
ic_cdk::export_candid!(); 