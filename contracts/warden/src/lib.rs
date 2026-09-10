#![no_std]

mod errors;
mod storage;
mod types;

use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

pub use errors::WardenError;
pub use types::{
    DataKey, Decision, EvaluationAllowedEvent, Policy, PolicySetEvent, RecipientTrustedEvent,
    RecipientUntrustedEvent, StepUpReason, StepupRequiredEvent, VelocityWindow,
};

const DAY_IN_LEDGERS: u32 = 17280;
const SECONDS_PER_DAY: u64 = 86400;
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

    pub fn evaluate(
        env: Env,
        wallet: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<Decision, WardenError> {
        wallet.require_auth();

        let policy =
            storage::read_policy(&env, &wallet).ok_or(WardenError::PolicyNotFound)?;

        if amount <= 0 {
            return Err(WardenError::InvalidAmount);
        }

        let now = env.ledger().timestamp();
        let mut window = storage::read_velocity(&env, &wallet).unwrap_or(VelocityWindow {
            window_start: 0,
            cumulative_amount: 0,
            tx_count: 0,
        });

        if now - window.window_start >= SECONDS_PER_DAY {
            window.window_start = now;
            window.cumulative_amount = 0;
            window.tx_count = 0;
        }

        let decision = if policy.new_recipient_requires_stepup
            && !policy.trusted_recipients.contains(&recipient)
        {
            Decision::RequireStepUp(StepUpReason::NewRecipient)
        } else if amount > policy.max_no_stepup {
            Decision::RequireStepUp(StepUpReason::AmountExceeded)
        } else if window.cumulative_amount + amount > policy.daily_velocity_cap {
            Decision::RequireStepUp(StepUpReason::VelocityExceeded)
        } else {
            Decision::Allow
        };

        // Velocity accumulates regardless of the decision reached: a transfer
        // that triggered step-up and was then completed by the user still
        // happened, and must count toward the cap. Only counting Allow-ed
        // transfers would let someone reset their effective velocity just by
        // making every transfer trigger step-up.
        window.cumulative_amount += amount;
        window.tx_count += 1;
        storage::write_velocity(&env, &wallet, &window);

        match &decision {
            Decision::Allow => {
                EvaluationAllowedEvent {
                    wallet: wallet.clone(),
                    recipient: recipient.clone(),
                    amount,
                }
                .publish(&env);
            }
            Decision::RequireStepUp(reason) => {
                StepupRequiredEvent {
                    wallet: wallet.clone(),
                    recipient: recipient.clone(),
                    amount,
                    reason: reason.clone(),
                }
                .publish(&env);
            }
        }

        Ok(decision)
    }
}

#[cfg(test)]
mod test;
