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
const SECONDS_PER_HOUR: u64 = 3600;
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
        hourly_velocity_cap: i128,
    ) -> Result<(), WardenError> {
        wallet.require_auth();

        // hourly_velocity_cap must sit between 0 and daily_velocity_cap --
        // allowing more per hour than per day would make the hourly window
        // meaningless (the daily cap would never be the binding constraint).
        if max_no_stepup < 0
            || daily_velocity_cap < max_no_stepup
            || hourly_velocity_cap < 0
            || hourly_velocity_cap > daily_velocity_cap
        {
            return Err(WardenError::InvalidPolicyParams);
        }

        let now = env.ledger().timestamp();

        let policy = match storage::read_policy(&env, &wallet) {
            Some(mut existing) => {
                existing.max_no_stepup = max_no_stepup;
                existing.daily_velocity_cap = daily_velocity_cap;
                existing.hourly_velocity_cap = hourly_velocity_cap;
                existing.new_recipient_requires_stepup = new_recipient_requires_stepup;
                existing.updated_at = now;
                existing
            }
            None => Policy {
                owner: wallet.clone(),
                max_no_stepup,
                daily_velocity_cap,
                hourly_velocity_cap,
                new_recipient_requires_stepup,
                trusted_recipients: Vec::new(&env),
                updated_at: now,
            },
        };

        storage::write_policy(&env, &wallet, &policy);

        // Uses #[contractevent] (current API) rather than the deprecated
        // env.events().publish. data_format = "vec" on PolicySetEvent keeps the
        // on-chain wire shape positional: topics ("policy_set", wallet), data
        // (max_no_stepup, daily_velocity_cap, hourly_velocity_cap,
        // new_recipient_requires_stepup), matching the exact shape
        // warden-monitor's decoder is spec'd against.
        PolicySetEvent {
            wallet,
            max_no_stepup,
            daily_velocity_cap,
            hourly_velocity_cap,
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

        let mut daily_window =
            storage::read_daily_velocity(&env, &wallet).unwrap_or(VelocityWindow {
                window_start: 0,
                cumulative_amount: 0,
                tx_count: 0,
            });
        if now - daily_window.window_start >= SECONDS_PER_DAY {
            daily_window.window_start = now;
            daily_window.cumulative_amount = 0;
            daily_window.tx_count = 0;
        }

        let mut hourly_window =
            storage::read_hourly_velocity(&env, &wallet).unwrap_or(VelocityWindow {
                window_start: 0,
                cumulative_amount: 0,
                tx_count: 0,
            });
        if now - hourly_window.window_start >= SECONDS_PER_HOUR {
            hourly_window.window_start = now;
            hourly_window.cumulative_amount = 0;
            hourly_window.tx_count = 0;
        }

        // Order: new recipient, then amount, then hourly velocity, then
        // daily velocity. When both windows are simultaneously exceeded,
        // HourlyVelocityExceeded is reported -- it's the more specific,
        // more immediately actionable signal ("you're moving too fast"
        // rather than "you hit your day limit").
        let decision = if policy.new_recipient_requires_stepup
            && !policy.trusted_recipients.contains(&recipient)
        {
            Decision::RequireStepUp(StepUpReason::NewRecipient)
        } else if amount > policy.max_no_stepup {
            Decision::RequireStepUp(StepUpReason::AmountExceeded)
        } else if hourly_window.cumulative_amount + amount > policy.hourly_velocity_cap {
            Decision::RequireStepUp(StepUpReason::HourlyVelocityExceeded)
        } else if daily_window.cumulative_amount + amount > policy.daily_velocity_cap {
            Decision::RequireStepUp(StepUpReason::VelocityExceeded)
        } else {
            Decision::Allow
        };

        // Both windows accumulate regardless of the decision reached: a
        // transfer that triggered step-up and was then completed by the user
        // still happened, and must count toward both caps. Only counting
        // Allow-ed transfers would let someone reset their effective
        // velocity just by making every transfer trigger step-up.
        daily_window.cumulative_amount += amount;
        daily_window.tx_count += 1;
        storage::write_daily_velocity(&env, &wallet, &daily_window);

        hourly_window.cumulative_amount += amount;
        hourly_window.tx_count += 1;
        storage::write_hourly_velocity(&env, &wallet, &hourly_window);

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

    pub fn get_policy(env: Env, wallet: Address) -> Result<Policy, WardenError> {
        storage::read_policy(&env, &wallet).ok_or(WardenError::PolicyNotFound)
    }

    pub fn get_velocity(env: Env, wallet: Address) -> Result<VelocityWindow, WardenError> {
        let window = storage::read_daily_velocity(&env, &wallet).unwrap_or(VelocityWindow {
            window_start: 0,
            cumulative_amount: 0,
            tx_count: 0,
        });
        Ok(window)
    }
}

#[cfg(test)]
mod test;
