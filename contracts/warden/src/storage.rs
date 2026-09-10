use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Policy};

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
