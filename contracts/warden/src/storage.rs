use soroban_sdk::{Address, Env};

use crate::types::{
    AccountState, DataKey, GuardianConfig, Policy, RecoveryProposal, VelocityWindow,
};

// Assumes an average ~5 second ledger close time.
const DAY_IN_LEDGERS: u32 = 17280;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = DAY_IN_LEDGERS * 30;
const PERSISTENT_BUMP_AMOUNT: u32 = DAY_IN_LEDGERS * 60;

pub fn read_policy(env: &Env, wallet: &Address) -> Option<Policy> {
    let key = DataKey::Policy(wallet.clone());
    env.storage().persistent().get(&key)
}

pub fn write_policy(env: &Env, wallet: &Address, policy: &Policy) {
    let key = DataKey::Policy(wallet.clone());
    env.storage().persistent().set(&key, policy);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

pub fn read_daily_velocity(env: &Env, wallet: &Address) -> Option<VelocityWindow> {
    let key = DataKey::Velocity(wallet.clone());
    env.storage().persistent().get(&key)
}

pub fn write_daily_velocity(env: &Env, wallet: &Address, velocity: &VelocityWindow) {
    let key = DataKey::Velocity(wallet.clone());
    env.storage().persistent().set(&key, velocity);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

pub fn read_hourly_velocity(env: &Env, wallet: &Address) -> Option<VelocityWindow> {
    let key = DataKey::HourlyVelocity(wallet.clone());
    env.storage().persistent().get(&key)
}

pub fn write_hourly_velocity(env: &Env, wallet: &Address, velocity: &VelocityWindow) {
    let key = DataKey::HourlyVelocity(wallet.clone());
    env.storage().persistent().set(&key, velocity);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

// Absence means not flagged -- no separate "does this key exist" check
// needed anywhere that calls this.
pub fn is_address_flagged(env: &Env, address: &Address) -> bool {
    let key = DataKey::FlaggedAddress(address.clone());
    env.storage().persistent().get(&key).unwrap_or(false)
}

pub fn write_flagged_address(env: &Env, address: &Address) {
    let key = DataKey::FlaggedAddress(address.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

pub fn remove_flagged_address(env: &Env, address: &Address) {
    let key = DataKey::FlaggedAddress(address.clone());
    env.storage().persistent().remove(&key);
}

// Absence means AccountState::Normal.
pub fn read_account_state(env: &Env, wallet: &Address) -> AccountState {
    let key = DataKey::AccountState(wallet.clone());
    env.storage().persistent().get(&key).unwrap_or(AccountState::Normal)
}

pub fn write_account_state(env: &Env, wallet: &Address, state: &AccountState) {
    let key = DataKey::AccountState(wallet.clone());
    env.storage().persistent().set(&key, state);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

pub fn read_guardian_config(env: &Env, wallet: &Address) -> Option<GuardianConfig> {
    let key = DataKey::GuardianConfig(wallet.clone());
    env.storage().persistent().get(&key)
}

pub fn write_guardian_config(env: &Env, wallet: &Address, config: &GuardianConfig) {
    let key = DataKey::GuardianConfig(wallet.clone());
    env.storage().persistent().set(&key, config);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

pub fn read_recovery_proposal(env: &Env, wallet: &Address) -> Option<RecoveryProposal> {
    let key = DataKey::RecoveryProposal(wallet.clone());
    env.storage().persistent().get(&key)
}

pub fn write_recovery_proposal(env: &Env, wallet: &Address, proposal: &RecoveryProposal) {
    let key = DataKey::RecoveryProposal(wallet.clone());
    env.storage().persistent().set(&key, proposal);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

pub fn remove_recovery_proposal(env: &Env, wallet: &Address) {
    let key = DataKey::RecoveryProposal(wallet.clone());
    env.storage().persistent().remove(&key);
}
