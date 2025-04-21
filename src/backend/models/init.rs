// src/backend/models/init.rs
use candid::{CandidType, Principal};
use serde::Deserialize;

#[derive(CandidType, Deserialize, Debug)]
pub struct InitArgs {
    pub admin_principal: Principal,
    pub cron_principal: Principal,
    pub min_cycles_threshold: u128,
} 