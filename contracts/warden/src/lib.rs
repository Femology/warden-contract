#![no_std]

mod errors;
mod storage;
mod types;

use soroban_sdk::{contract, contractimpl, Address, Env, Map};

pub use errors::WardenError;
pub use types::{
    AddressFlaggedEvent, AddressUnflaggedEvent, DataKey, Decision, EvaluationAllowedEvent, Policy,
    PolicySetEvent, RecipientTrustedEvent, RecipientUntrustedEvent, StepUpReason,
    StepupRequiredEvent, VelocityWindow,
};

const DAY_IN_LEDGERS: u32 = 17280;
const SECONDS_PER_DAY: u64 = 86400;
const SECONDS_PER_HOUR: u64 = 3600;
const INSTANCE_LIFETIME_THRESHOLD: u32 = DAY_IN_LEDGERS * 30;
const INSTANCE_BUMP_AMOUNT: u32 = DAY_IN_LEDGERS * 60;

// Proves both identity (the caller really is `admin`, via require_auth) and
// privilege (that address is the one stored at initialize) -- require_auth()
// alone only proves the former. Not a #[contractimpl] method: it's an
// internal helper, not a contract entry point.
fn require_admin(env: &Env, admin: &Address) -> Result<(), WardenError> {
    admin.require_auth();

    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(WardenError::NotInitialized)?;

    if stored_admin != *admin {
        return Err(WardenError::NotAdmin);
    }

    Ok(())
}

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
        trust_decay_seconds: u64,
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
                existing.trust_decay_seconds = trust_decay_seconds;
                existing.updated_at = now;
                existing
            }
            None => Policy {
                owner: wallet.clone(),
                max_no_stepup,
                daily_velocity_cap,
                hourly_velocity_cap,
                new_recipient_requires_stepup,
                trusted_recipients: Map::new(&env),
                trust_decay_seconds,
                updated_at: now,
            },
        };

        storage::write_policy(&env, &wallet, &policy);

        // Uses #[contractevent] (current API) rather than the deprecated
        // env.events().publish. data_format = "vec" on PolicySetEvent keeps the
        // on-chain wire shape positional: topics ("policy_set", wallet), data
        // (max_no_stepup, daily_velocity_cap, hourly_velocity_cap,
        // new_recipient_requires_stepup, trust_decay_seconds), matching the
        // exact shape warden-monitor's decoder is spec'd against.
        PolicySetEvent {
            wallet,
            max_no_stepup,
            daily_velocity_cap,
            hourly_velocity_cap,
            new_recipient_requires_stepup,
            trust_decay_seconds,
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

        if policy.trusted_recipients.contains_key(recipient.clone()) {
            return Err(WardenError::RecipientAlreadyTrusted);
        }

        let now = env.ledger().timestamp();
        // last_paid_at starts at "now", not zero: trust begins fresh the
        // moment it's granted, rather than immediately reading as decayed.
        policy.trusted_recipients.set(recipient.clone(), now);
        policy.updated_at = now;

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

        if policy.trusted_recipients.remove(recipient.clone()).is_none() {
            return Err(WardenError::RecipientNotTrusted);
        }
        policy.updated_at = env.ledger().timestamp();

        storage::write_policy(&env, &wallet, &policy);

        RecipientUntrustedEvent { wallet, recipient }.publish(&env);

        Ok(())
    }

    pub fn add_flagged_address(
        env: Env,
        admin: Address,
        address: Address,
    ) -> Result<(), WardenError> {
        require_admin(&env, &admin)?;

        if storage::is_address_flagged(&env, &address) {
            return Err(WardenError::AddressAlreadyFlagged);
        }

        storage::write_flagged_address(&env, &address);

        AddressFlaggedEvent { admin, address }.publish(&env);

        Ok(())
    }

    pub fn remove_flagged_address(
        env: Env,
        admin: Address,
        address: Address,
    ) -> Result<(), WardenError> {
        require_admin(&env, &admin)?;

        if !storage::is_address_flagged(&env, &address) {
            return Err(WardenError::AddressNotFlagged);
        }

        storage::remove_flagged_address(&env, &address);

        AddressUnflaggedEvent { admin, address }.publish(&env);

        Ok(())
    }

    pub fn evaluate(
        env: Env,
        wallet: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<Decision, WardenError> {
        wallet.require_auth();

        let mut policy =
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

        // A recipient counts as trusted only if they're in the map AND
        // haven't gone longer than trust_decay_seconds since last_paid_at --
        // still present in the list either way (add/remove don't care about
        // decay), just no longer skipping the new-recipient check.
        let is_actively_trusted = match policy.trusted_recipients.get(recipient.clone()) {
            Some(last_paid_at) => now.saturating_sub(last_paid_at) <= policy.trust_decay_seconds,
            None => false,
        };

        // Order: flagged recipient first, then new recipient, then amount,
        // then hourly velocity, then daily velocity. Flagged wins over
        // everything else -- a flagged address always needs step-up
        // regardless of amount, trust, or velocity headroom, per the
        // registry's whole purpose. This check runs after the policy is
        // loaded (a wallet with no policy still gets PolicyNotFound, flagged
        // recipient or not) but before any of the policy-dependent checks,
        // and doesn't consult trusted_recipients/velocity at all -- a
        // flagged address's own history with this wallet is irrelevant.
        // When both windows are simultaneously exceeded, HourlyVelocityExceeded
        // is reported -- it's the more specific, more immediately actionable
        // signal ("you're moving too fast" rather than "you hit your day
        // limit").
        let decision = if storage::is_address_flagged(&env, &recipient) {
            Decision::RequireStepUp(StepUpReason::FlaggedRecipient)
        } else if policy.new_recipient_requires_stepup && !is_actively_trusted {
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

        // Refresh last_paid_at for an existing trusted recipient on every
        // evaluate() call, regardless of the decision -- same reasoning as
        // the velocity windows above: a transfer that happened (even one
        // that required step-up) is real evidence this recipient is still
        // actively being paid, and should reset their decay clock. This
        // does NOT touch updated_at (that field means "the owner changed
        // their policy configuration", not "a payment happened") and does
        // NOT create an entry for a recipient who isn't already trusted --
        // the only way to start trusting someone is add_trusted_recipient.
        if policy.trusted_recipients.contains_key(recipient.clone()) {
            policy.trusted_recipients.set(recipient.clone(), now);
            storage::write_policy(&env, &wallet, &policy);
        }

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
