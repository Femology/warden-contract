#![no_std]

mod errors;
mod storage;
mod types;

use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

pub use errors::WardenError;
pub use types::{
    DataKey, Decision, Policy, PolicySetEvent, RecipientTrustedEvent, RecipientUntrustedEvent,
    StepUpReason, VelocityWindow,
};

const DAY_IN_LEDGERS: u32 = 17280;
const INSTANCE_LIFETIME_THRESHOLD: u32 = DAY_IN_LEDGERS * 30;
const INSTANCE_BUMP_AMOUNT: u32 = DAY_IN_LEDGERS * 60;

#[contract]
pub struct WardenContract;

#[contractimpl]
impl WardenContract {
    pub fn initialize(env: Env, admin: Address, reference_asset: Address) -> Result<(), WardenError> {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            return Err(WardenError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::ReferenceAsset, &reference_asset);
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        Ok(())
    }

    pub fn set_policy(
        env: Env,
        wallet: Address,
        max_no_stepup: i128,
        daily_velocity_cap: i128,
        new_recipient_requires_stepup: bool,
    ) -> Result<(), WardenError> {
        wallet.require_auth();

        if max_no_stepup < 0 || daily_velocity_cap < max_no_stepup {
            return Err(WardenError::InvalidPolicyParams);
        }

        let now = env.ledger().timestamp();

        let policy = match storage::read_policy(&env, &wallet) {
            Some(mut existing) => {
                existing.max_no_stepup = max_no_stepup;
                existing.daily_velocity_cap = daily_velocity_cap;
                existing.new_recipient_requires_stepup = new_recipient_requires_stepup;
                existing.updated_at = now;
                existing
            }
            None => Policy {
                owner: wallet.clone(),
                max_no_stepup,
                daily_velocity_cap,
                new_recipient_requires_stepup,
                trusted_recipients: Vec::new(&env),
                updated_at: now,
            },
        };

        storage::write_policy(&env, &wallet, &policy);

        // Uses #[contractevent] (current API) rather than the deprecated
        // env.events().publish. data_format = "vec" on PolicySetEvent keeps the
        // on-chain wire shape positional: topics ("policy_set", wallet), data
        // (max_no_stepup, daily_velocity_cap, new_recipient_requires_stepup),
        // matching the exact shape warden-monitor's decoder is spec'd against.
        PolicySetEvent {
            wallet,
            max_no_stepup,
            daily_velocity_cap,
            new_recipient_requires_stepup,
        }
        .publish(&env);

        Ok(())
    }

    pub fn add_trusted_recipient(
        env: Env,
        wallet: Address,
        recipient: Address,
    ) -> Result<(), WardenError> {
        wallet.require_auth();

        let mut policy =
            storage::read_policy(&env, &wallet).ok_or(WardenError::PolicyNotFound)?;

        if policy.trusted_recipients.contains(&recipient) {
            return Err(WardenError::RecipientAlreadyTrusted);
        }

        policy.trusted_recipients.push_back(recipient.clone());
        policy.updated_at = env.ledger().timestamp();

        storage::write_policy(&env, &wallet, &policy);

        RecipientTrustedEvent { wallet, recipient }.publish(&env);

        Ok(())
    }

    pub fn remove_trusted_recipient(
        env: Env,
        wallet: Address,
        recipient: Address,
    ) -> Result<(), WardenError> {
        wallet.require_auth();

        let mut policy =
            storage::read_policy(&env, &wallet).ok_or(WardenError::PolicyNotFound)?;

        let index = policy
            .trusted_recipients
            .first_index_of(&recipient)
            .ok_or(WardenError::RecipientNotTrusted)?;

        policy.trusted_recipients.remove(index);
        policy.updated_at = env.ledger().timestamp();

        storage::write_policy(&env, &wallet, &policy);

        RecipientUntrustedEvent { wallet, recipient }.publish(&env);

        Ok(())
    }
}

#[cfg(test)]
mod test;
