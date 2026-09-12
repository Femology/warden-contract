#![cfg(test)]

use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env, Vec};

use crate::{
    storage, AccountState, Decision, StepUpReason, WardenContract, WardenContractClient,
    WardenError,
};

fn setup<'a>() -> (Env, WardenContractClient<'a>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WardenContract, ());
    let client = WardenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let reference_asset = Address::generate(&env);

    (env, client, contract_id, admin, reference_asset)
}

#[test]
fn initialize_succeeds_on_first_call() {
    let (_env, client, _contract_id, admin, reference_asset) = setup();

    client.initialize(&admin, &reference_asset);
}

#[test]
fn initialize_fails_when_already_initialized() {
    let (_env, client, _contract_id, admin, reference_asset) = setup();

    client.initialize(&admin, &reference_asset);

    let result = client.try_initialize(&admin, &reference_asset);
    assert_eq!(result, Err(Ok(WardenError::AlreadyInitialized)));
}

#[test]
fn set_policy_creates_new_policy() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000, &999_999_999);

    let policy = env
        .as_contract(&contract_id, || storage::read_policy(&env, &wallet))
        .expect("policy should exist after set_policy");

    assert_eq!(policy.owner, wallet);
    assert_eq!(policy.max_no_stepup, 1_000);
    assert_eq!(policy.daily_velocity_cap, 5_000);
    assert_eq!(policy.hourly_velocity_cap, 5_000);
    assert!(policy.new_recipient_requires_stepup);
    assert_eq!(policy.trusted_recipients.len(), 0);
}

#[test]
fn set_policy_rejects_hourly_velocity_cap_above_daily_cap() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let result = client.try_set_policy(&wallet, &1_000, &5_000, &true, &6_000, &999_999_999);
    assert_eq!(result, Err(Ok(WardenError::InvalidPolicyParams)));
}

#[test]
fn set_policy_updates_existing_policy_without_touching_trusted_recipients() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000, &999_999_999);

    // Seed a trusted recipient directly through the storage layer, since
    // add_trusted_recipient does not exist yet at this point in the build
    // sequence (it lands in the next commit). This test only needs to prove
    // that set_policy leaves an existing trusted_recipients list untouched.
    env.as_contract(&contract_id, || {
        let mut policy = storage::read_policy(&env, &wallet).unwrap();
        policy.trusted_recipients.set(recipient.clone(), 42);
        storage::write_policy(&env, &wallet, &policy);
    });

    client.set_policy(&wallet, &2_000, &9_000, &false, &9_000, &999_999_999);

    let policy = env
        .as_contract(&contract_id, || storage::read_policy(&env, &wallet))
        .expect("policy should exist after update");

    assert_eq!(policy.max_no_stepup, 2_000);
    assert_eq!(policy.daily_velocity_cap, 9_000);
    assert!(!policy.new_recipient_requires_stepup);
    assert_eq!(policy.trusted_recipients.len(), 1);
    assert_eq!(policy.trusted_recipients.get(recipient).unwrap(), 42);
}

#[test]
fn set_policy_rejects_negative_max_no_stepup() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let result = client.try_set_policy(&wallet, &-1, &5_000, &true, &5_000, &999_999_999);
    assert_eq!(result, Err(Ok(WardenError::InvalidPolicyParams)));
}

#[test]
fn set_policy_rejects_velocity_cap_below_max_no_stepup() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let result = client.try_set_policy(&wallet, &5_000, &1_000, &true, &1_000, &999_999_999);
    assert_eq!(result, Err(Ok(WardenError::InvalidPolicyParams)));
}

#[test]
fn add_trusted_recipient_succeeds() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000, &999_999_999);

    let before = env.ledger().timestamp();
    client.add_trusted_recipient(&wallet, &recipient);

    let policy = env
        .as_contract(&contract_id, || storage::read_policy(&env, &wallet))
        .unwrap();

    assert_eq!(policy.trusted_recipients.len(), 1);
    assert_eq!(
        policy.trusted_recipients.get(recipient).unwrap(),
        before,
        "last_paid_at should be set to the ledger time it was added"
    );
}

#[test]
fn add_trusted_recipient_fails_when_already_trusted() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000, &999_999_999);
    client.add_trusted_recipient(&wallet, &recipient);

    let result = client.try_add_trusted_recipient(&wallet, &recipient);
    assert_eq!(result, Err(Ok(WardenError::RecipientAlreadyTrusted)));
}

#[test]
fn add_trusted_recipient_fails_when_no_policy() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = client.try_add_trusted_recipient(&wallet, &recipient);
    assert_eq!(result, Err(Ok(WardenError::PolicyNotFound)));
}

#[test]
fn remove_trusted_recipient_succeeds() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000, &999_999_999);
    client.add_trusted_recipient(&wallet, &recipient);
    client.remove_trusted_recipient(&wallet, &recipient);

    let policy = env
        .as_contract(&contract_id, || storage::read_policy(&env, &wallet))
        .unwrap();

    assert_eq!(policy.trusted_recipients.len(), 0);
}

#[test]
fn remove_trusted_recipient_fails_when_not_trusted() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000, &999_999_999);

    let result = client.try_remove_trusted_recipient(&wallet, &recipient);
    assert_eq!(result, Err(Ok(WardenError::RecipientNotTrusted)));
}

#[test]
fn set_policy_emits_policy_set_event_with_exact_topic_and_data_shape() {
    use soroban_sdk::{testutils::Events as _, vec as svec, IntoVal, Symbol, Val};

    let (env, client, contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);

    client.set_policy(&wallet, &1_000i128, &5_000i128, &true, &5_000i128, &999_999_999);

    let mut data: soroban_sdk::Vec<Val> = soroban_sdk::Vec::new(&env);
    data.push_back(1_000i128.into_val(&env));
    data.push_back(5_000i128.into_val(&env));
    data.push_back(5_000i128.into_val(&env));
    data.push_back(true.into_val(&env));
    data.push_back(999_999_999u64.into_val(&env));

    assert_eq!(
        env.events().all(),
        svec![
            &env,
            (
                contract_id.clone(),
                (Symbol::new(&env, "policy_set"), wallet.clone()).into_val(&env),
                data.into_val(&env),
            ),
        ]
    );
}

#[test]
fn evaluate_allows_when_under_thresholds() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000, &999_999_999);

    let decision = client.evaluate(&wallet, &recipient, &500);
    assert_eq!(decision, Decision::Allow);
}

#[test]
fn evaluate_requires_stepup_for_new_recipient() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000, &999_999_999);

    let decision = client.evaluate(&wallet, &recipient, &500);
    assert_eq!(decision, Decision::RequireStepUp(StepUpReason::NewRecipient));
}

#[test]
fn evaluate_requires_stepup_when_amount_exceeds_max() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000, &999_999_999);

    let decision = client.evaluate(&wallet, &recipient, &1_500);
    assert_eq!(
        decision,
        Decision::RequireStepUp(StepUpReason::AmountExceeded)
    );
}

#[test]
fn evaluate_requires_stepup_when_velocity_cap_exceeded() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &1_500, &false, &1_500, &999_999_999);

    // First transfer: under max_no_stepup and under the daily cap -> Allow.
    let start = env.ledger().timestamp();
    let first = client.evaluate(&wallet, &recipient, &1_000);
    assert_eq!(first, Decision::Allow);

    // Advance past the hourly window's reset (3600s) but well within the
    // daily window's (86400s). hourly_velocity_cap equals daily_velocity_cap
    // here, so without this the hourly window (checked first) would trip
    // before the daily one -- advancing lets the hourly window reset
    // independently, isolating the daily cap this test is actually about.
    env.ledger().set_timestamp(start + 3_601);

    // Second transfer: 600 is under max_no_stepup, and under the now-reset
    // hourly cap alone (0 + 600 <= 1500), but cumulative daily spend
    // (1000 + 600 = 1600) still exceeds the daily cap of 1500.
    let second = client.evaluate(&wallet, &recipient, &600);
    assert_eq!(
        second,
        Decision::RequireStepUp(StepUpReason::VelocityExceeded)
    );
}

#[test]
fn evaluate_velocity_window_resets_after_24h() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &1_000, &false, &1_000, &999_999_999);

    let start = env.ledger().timestamp();

    let first = client.evaluate(&wallet, &recipient, &1_000);
    assert_eq!(first, Decision::Allow);

    // Advance past the hourly reset but well within the day -- same
    // isolation technique as the test above, so this test's own hourly cap
    // (equal to the daily cap) doesn't confound the daily-reset behavior
    // this test is actually about.
    env.ledger().set_timestamp(start + 3_601);

    let blocked = client.evaluate(&wallet, &recipient, &1_000);
    assert_eq!(
        blocked,
        Decision::RequireStepUp(StepUpReason::VelocityExceeded)
    );

    // Advance to a full day past the original window start: the daily
    // window resets (the hourly window has reset many times over by now
    // too, which is expected and irrelevant to what this test checks).
    env.ledger().set_timestamp(start + 86_400);

    let after_reset = client.evaluate(&wallet, &recipient, &1_000);
    assert_eq!(after_reset, Decision::Allow);
}

#[test]
fn evaluate_velocity_accumulates_even_when_stepup_required() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000, &999_999_999);

    // This exceeds max_no_stepup, so it triggers step-up...
    let decision = client.evaluate(&wallet, &recipient, &2_000);
    assert_eq!(
        decision,
        Decision::RequireStepUp(StepUpReason::AmountExceeded)
    );

    // ...but the velocity window must still record the full amount. Otherwise
    // someone could reset their effective velocity cap just by making every
    // transfer trigger step-up.
    let window = env
        .as_contract(&contract_id, || storage::read_daily_velocity(&env, &wallet))
        .unwrap();
    assert_eq!(window.cumulative_amount, 2_000);
    assert_eq!(window.tx_count, 1);
}

#[test]
fn evaluate_fails_when_no_policy() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = client.try_evaluate(&wallet, &recipient, &100);
    assert_eq!(result, Err(Ok(WardenError::PolicyNotFound)));
}

#[test]
fn evaluate_fails_when_amount_not_positive() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000, &999_999_999);

    let zero_result = client.try_evaluate(&wallet, &recipient, &0);
    assert_eq!(zero_result, Err(Ok(WardenError::InvalidAmount)));

    let negative_result = client.try_evaluate(&wallet, &recipient, &-5);
    assert_eq!(negative_result, Err(Ok(WardenError::InvalidAmount)));
}

#[test]
fn get_policy_returns_configured_policy() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000, &999_999_999);

    let policy = client.get_policy(&wallet);
    assert_eq!(policy.max_no_stepup, 1_000);
    assert_eq!(policy.daily_velocity_cap, 5_000);
    assert!(policy.new_recipient_requires_stepup);
}

#[test]
fn get_policy_fails_when_not_set() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let result = client.try_get_policy(&wallet);
    assert_eq!(result, Err(Ok(WardenError::PolicyNotFound)));
}

#[test]
fn get_velocity_returns_recorded_window() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000, &999_999_999);
    client.evaluate(&wallet, &recipient, &400);

    let window = client.get_velocity(&wallet);
    assert_eq!(window.cumulative_amount, 400);
    assert_eq!(window.tx_count, 1);
}

#[test]
fn get_velocity_returns_zeroed_window_when_no_activity() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let window = client.get_velocity(&wallet);

    assert_eq!(window.window_start, 0);
    assert_eq!(window.cumulative_amount, 0);
    assert_eq!(window.tx_count, 0);
}

#[test]
fn evaluate_requires_stepup_for_hourly_velocity_while_daily_has_headroom() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Daily cap is generous (10_000); hourly cap is tight (500). Neither
    // transfer below is anywhere near the daily cap.
    client.set_policy(&wallet, &1_000, &10_000, &false, &500, &999_999_999);

    let first = client.evaluate(&wallet, &recipient, &300);
    assert_eq!(first, Decision::Allow);

    // Cumulative is now 600 for both windows: 600 <= 10_000 (daily headroom
    // untouched) but 600 > 500 (hourly cap exceeded). This is the case the
    // hourly window exists for -- rapid spending a single daily cap alone
    // would not catch until far more had been spent.
    let second = client.evaluate(&wallet, &recipient, &300);
    assert_eq!(
        second,
        Decision::RequireStepUp(StepUpReason::HourlyVelocityExceeded)
    );

    let daily = env
        .as_contract(&_contract_id, || storage::read_daily_velocity(&env, &wallet))
        .unwrap();
    assert_eq!(daily.cumulative_amount, 600);
    assert!(daily.cumulative_amount < 10_000);
}

#[test]
fn evaluate_recipient_decays_out_of_trust_after_configured_period() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    // trust_decay_seconds = 100: short enough to advance past in a test.
    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000, &100);
    client.add_trusted_recipient(&wallet, &recipient);

    // Immediately after trusting: still fresh, well within the decay
    // window -> Allow.
    let fresh = client.evaluate(&wallet, &recipient, &10);
    assert_eq!(fresh, Decision::Allow);

    // Advance past trust_decay_seconds without paying this recipient again.
    let now = env.ledger().timestamp();
    env.ledger().set_timestamp(now + 101);

    // Still in trusted_recipients (add/remove never touched it), but too
    // long since last_paid_at -> treated the same as a brand-new recipient.
    let decayed = client.evaluate(&wallet, &recipient, &10);
    assert_eq!(decayed, Decision::RequireStepUp(StepUpReason::NewRecipient));
}

#[test]
fn add_flagged_address_succeeds_for_the_real_admin() {
    let (env, client, contract_id, admin, reference_asset) = setup();
    client.initialize(&admin, &reference_asset);

    let flagged = Address::generate(&env);
    client.add_flagged_address(&admin, &flagged);

    let is_flagged = env.as_contract(&contract_id, || storage::is_address_flagged(&env, &flagged));
    assert!(is_flagged);
}

#[test]
fn add_flagged_address_fails_for_a_non_admin_address() {
    let (env, client, _contract_id, admin, reference_asset) = setup();
    client.initialize(&admin, &reference_asset);

    // mock_all_auths() makes every address's require_auth() succeed --
    // proving this fails only if the contract itself checks the caller
    // against the stored admin, not just that *someone* authorized the call.
    let impostor = Address::generate(&env);
    let target = Address::generate(&env);

    let result = client.try_add_flagged_address(&impostor, &target);
    assert_eq!(result, Err(Ok(WardenError::NotAdmin)));
}

#[test]
fn remove_flagged_address_fails_for_a_non_admin_address() {
    let (env, client, _contract_id, admin, reference_asset) = setup();
    client.initialize(&admin, &reference_asset);

    let target = Address::generate(&env);
    client.add_flagged_address(&admin, &target);

    let impostor = Address::generate(&env);
    let result = client.try_remove_flagged_address(&impostor, &target);
    assert_eq!(result, Err(Ok(WardenError::NotAdmin)));
}

#[test]
fn flagged_address_functions_fail_before_initialize() {
    let (env, client, _contract_id, admin, _reference_asset) = setup();

    let target = Address::generate(&env);
    let result = client.try_add_flagged_address(&admin, &target);
    assert_eq!(result, Err(Ok(WardenError::NotInitialized)));
}

#[test]
fn add_flagged_address_fails_when_already_flagged() {
    let (env, client, _contract_id, admin, reference_asset) = setup();
    client.initialize(&admin, &reference_asset);

    let target = Address::generate(&env);
    client.add_flagged_address(&admin, &target);

    let result = client.try_add_flagged_address(&admin, &target);
    assert_eq!(result, Err(Ok(WardenError::AddressAlreadyFlagged)));
}

#[test]
fn remove_flagged_address_succeeds_and_clears_the_flag() {
    let (env, client, contract_id, admin, reference_asset) = setup();
    client.initialize(&admin, &reference_asset);

    let target = Address::generate(&env);
    client.add_flagged_address(&admin, &target);
    client.remove_flagged_address(&admin, &target);

    let is_flagged = env.as_contract(&contract_id, || storage::is_address_flagged(&env, &target));
    assert!(!is_flagged);
}

#[test]
fn remove_flagged_address_fails_when_not_flagged() {
    let (env, client, _contract_id, admin, reference_asset) = setup();
    client.initialize(&admin, &reference_asset);

    let target = Address::generate(&env);
    let result = client.try_remove_flagged_address(&admin, &target);
    assert_eq!(result, Err(Ok(WardenError::AddressNotFlagged)));
}

#[test]
fn evaluate_requires_stepup_for_a_flagged_recipient_regardless_of_amount_trust_or_velocity() {
    let (env, client, _contract_id, admin, reference_asset) = setup();
    client.initialize(&admin, &reference_asset);

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Trusted, small amount, well inside every velocity cap -- every other
    // signal says Allow. new_recipient_requires_stepup is even off.
    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000, &999_999_999);
    client.add_trusted_recipient(&wallet, &recipient);

    let before_flagging = client.evaluate(&wallet, &recipient, &10);
    assert_eq!(before_flagging, Decision::Allow);

    client.add_flagged_address(&admin, &recipient);

    let decision = client.evaluate(&wallet, &recipient, &10);
    assert_eq!(
        decision,
        Decision::RequireStepUp(StepUpReason::FlaggedRecipient)
    );
}

#[test]
fn evaluate_stops_requiring_stepup_once_a_flagged_address_is_unflagged() {
    let (env, client, _contract_id, admin, reference_asset) = setup();
    client.initialize(&admin, &reference_asset);

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000, &999_999_999);
    client.add_flagged_address(&admin, &recipient);

    let flagged = client.evaluate(&wallet, &recipient, &10);
    assert_eq!(
        flagged,
        Decision::RequireStepUp(StepUpReason::FlaggedRecipient)
    );

    client.remove_flagged_address(&admin, &recipient);

    let unflagged = client.evaluate(&wallet, &recipient, &10);
    assert_eq!(unflagged, Decision::Allow);
}

#[test]
fn evaluate_still_fails_with_policy_not_found_for_a_flagged_recipient() {
    // The flagged check is checked first among the *decision* branches, but
    // still comes after PolicyNotFound -- a wallet with no policy can't be
    // evaluated at all, flagged recipient or not.
    let (env, client, _contract_id, admin, reference_asset) = setup();
    client.initialize(&admin, &reference_asset);

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.add_flagged_address(&admin, &recipient);

    let result = client.try_evaluate(&wallet, &recipient, &10);
    assert_eq!(result, Err(Ok(WardenError::PolicyNotFound)));
}

fn addr_vec(env: &Env, addrs: &[&Address]) -> Vec<Address> {
    let mut v = Vec::new(env);
    for a in addrs {
        v.push_back((*a).clone());
    }
    v
}

// --- Phase 16: guardian and recovery subsystem -----------------------------

#[test]
fn set_guardians_fails_while_restricted_or_worse() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let guardian = Address::generate(&env);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });

    let guardians = addr_vec(&env, &[&guardian]);
    let result = client.try_set_guardians(&wallet, &guardians, &1u32);
    assert_eq!(result, Err(Ok(WardenError::GuardianConfigLocked)));

    // Worse than Restricted fails the same way.
    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Frozen);
    });
    let result = client.try_set_guardians(&wallet, &guardians, &1u32);
    assert_eq!(result, Err(Ok(WardenError::GuardianConfigLocked)));
}

#[test]
fn set_guardians_succeeds_while_normal_or_watch() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let guardians = addr_vec(&env, &[&g1, &g2]);

    // Default state is Normal -- no seeding needed.
    client.set_guardians(&wallet, &guardians, &2u32);

    let config = env
        .as_contract(&contract_id, || storage::read_guardian_config(&env, &wallet))
        .expect("guardian config should exist");
    assert_eq!(config.guardians.len(), 2);
    assert_eq!(config.threshold, 2);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Watch);
    });
    client.set_guardians(&wallet, &guardians, &1u32);
}

#[test]
fn set_guardians_rejects_invalid_configs() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);

    let mut too_many = Vec::new(&env);
    for _ in 0..8 {
        too_many.push_back(Address::generate(&env));
    }
    assert_eq!(
        client.try_set_guardians(&wallet, &too_many, &1u32),
        Err(Ok(WardenError::InvalidGuardianConfig))
    );

    let three = addr_vec(
        &env,
        &[
            &Address::generate(&env),
            &Address::generate(&env),
            &Address::generate(&env),
        ],
    );
    assert_eq!(
        client.try_set_guardians(&wallet, &three, &0u32),
        Err(Ok(WardenError::InvalidGuardianConfig))
    );
    assert_eq!(
        client.try_set_guardians(&wallet, &three, &4u32),
        Err(Ok(WardenError::InvalidGuardianConfig))
    );
}

#[test]
fn propose_recovery_creates_a_proposal_counting_the_proposers_own_approval() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&g1, &g2]), &2u32);

    // Simulates the account having been restricted by some earlier event
    // (an admin flag, or -- once built -- an oracle escalation). There's no
    // public function that escalates state yet, so this is seeded directly,
    // the same way other tests seed preconditions the public API can't
    // reach on its own.
    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });

    client.propose_recovery(&wallet, &g1, &AccountState::Normal);

    let proposal = env
        .as_contract(&contract_id, || storage::read_recovery_proposal(&env, &wallet))
        .expect("proposal should exist");
    assert_eq!(proposal.proposer, g1);
    assert_eq!(proposal.target_state, AccountState::Normal);
    assert_eq!(proposal.approvals.len(), 1);
    assert!(proposal.approvals.contains(&g1));
}

#[test]
fn propose_recovery_fails_for_a_non_guardian() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let guardian = Address::generate(&env);
    let outsider = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&guardian]), &1u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });

    let result = client.try_propose_recovery(&wallet, &outsider, &AccountState::Normal);
    assert_eq!(result, Err(Ok(WardenError::NotGuardian)));
}

#[test]
fn propose_recovery_fails_when_target_state_is_not_less_restrictive() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let guardian = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&guardian]), &1u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Watch);
    });

    // Same state as current -- not strictly less restrictive.
    let same = client.try_propose_recovery(&wallet, &guardian, &AccountState::Watch);
    assert_eq!(same, Err(Ok(WardenError::InvalidTargetState)));

    // More restrictive than current -- guardians can only ever move toward
    // less restriction, never more.
    let worse = client.try_propose_recovery(&wallet, &guardian, &AccountState::Frozen);
    assert_eq!(worse, Err(Ok(WardenError::InvalidTargetState)));
}

#[test]
fn propose_recovery_fails_when_one_is_already_pending() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&g1, &g2]), &2u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });

    client.propose_recovery(&wallet, &g1, &AccountState::Normal);

    let result = client.try_propose_recovery(&wallet, &g2, &AccountState::Watch);
    assert_eq!(result, Err(Ok(WardenError::RecoveryAlreadyProposed)));
}

#[test]
fn approve_recovery_fails_for_a_non_guardian() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let guardian = Address::generate(&env);
    let outsider = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&guardian]), &1u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });
    client.propose_recovery(&wallet, &guardian, &AccountState::Normal);

    let result = client.try_approve_recovery(&wallet, &outsider);
    assert_eq!(result, Err(Ok(WardenError::NotGuardian)));
}

#[test]
fn approve_recovery_fails_when_the_same_guardian_approves_twice() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&g1, &g2]), &2u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });
    // g1's own approval is already counted by propose_recovery.
    client.propose_recovery(&wallet, &g1, &AccountState::Normal);

    let result = client.try_approve_recovery(&wallet, &g1);
    assert_eq!(result, Err(Ok(WardenError::AlreadyApproved)));
}

#[test]
fn execute_recovery_fails_with_insufficient_approvals() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&g1, &g2, &g3]), &3u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });
    client.propose_recovery(&wallet, &g1, &AccountState::Normal);
    client.approve_recovery(&wallet, &g2);
    // Only 2 of the required 3 approvals.

    let now = env.ledger().timestamp();
    env.ledger().set_timestamp(now + 172_801);

    let result = client.try_execute_recovery(&wallet);
    assert_eq!(result, Err(Ok(WardenError::InsufficientApprovals)));
}

#[test]
fn execute_recovery_fails_before_timelock_elapses() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&g1, &g2]), &2u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });
    client.propose_recovery(&wallet, &g1, &AccountState::Normal);
    client.approve_recovery(&wallet, &g2);
    // Threshold (2) is met, but no time has passed -- the 48h timelock
    // hasn't elapsed yet.

    let result = client.try_execute_recovery(&wallet);
    assert_eq!(result, Err(Ok(WardenError::TimelockNotElapsed)));
}

#[test]
fn execute_recovery_succeeds_without_wallet_signature_once_threshold_and_timelock_are_met() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&g1, &g2]), &2u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });
    client.propose_recovery(&wallet, &g1, &AccountState::Normal);
    client.approve_recovery(&wallet, &g2);

    let proposed_at = env.ledger().timestamp();
    env.ledger().set_timestamp(proposed_at + 172_801);

    // The whole point of this test: mock_auths(&[]) authorizes *nothing* for
    // this one call, overriding setup()'s blanket mock_all_auths() for just
    // this invocation. If execute_recovery called wallet.require_auth() (or
    // any require_auth at all, on any address), this would panic on a
    // missing authorization instead of succeeding -- so a passing test here
    // is real proof no signature was required, not just an assumption from
    // reading the source.
    client.mock_auths(&[]).execute_recovery(&wallet);

    let state = env.as_contract(&contract_id, || storage::read_account_state(&env, &wallet));
    assert_eq!(state, AccountState::Normal);

    let proposal = env.as_contract(&contract_id, || storage::read_recovery_proposal(&env, &wallet));
    assert!(proposal.is_none());
}

#[test]
fn execute_recovery_fails_when_no_proposal_is_pending() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let guardian = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&guardian]), &1u32);

    let result = client.try_execute_recovery(&wallet);
    assert_eq!(result, Err(Ok(WardenError::RecoveryNotFound)));
}

#[test]
fn cancel_recovery_lets_the_owner_cancel_a_pending_proposal_with_their_own_signature() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&g1, &g2]), &2u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });
    client.propose_recovery(&wallet, &g1, &AccountState::Normal);
    client.approve_recovery(&wallet, &g2);

    // The owner vetoes it -- protects against a guardian-majority collusion
    // attack while the owner's own key is still fine. Uses the real client
    // call (under setup()'s mock_all_auths()), which specifically means
    // wallet.require_auth() succeeded -- this is the owner's own signature,
    // not anyone else's.
    client.cancel_recovery(&wallet);

    let proposal = env.as_contract(&contract_id, || storage::read_recovery_proposal(&env, &wallet));
    assert!(proposal.is_none());

    // Threshold and timelock no longer matter -- there's nothing left to
    // execute.
    let now = env.ledger().timestamp();
    env.ledger().set_timestamp(now + 172_801);
    let result = client.try_execute_recovery(&wallet);
    assert_eq!(result, Err(Ok(WardenError::RecoveryNotFound)));

    // The account state was never actually changed by the vetoed proposal.
    let state = env.as_contract(&contract_id, || storage::read_account_state(&env, &wallet));
    assert_eq!(state, AccountState::Restricted);
}

#[test]
fn cancel_recovery_fails_when_nothing_is_pending() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);

    let result = client.try_cancel_recovery(&wallet);
    assert_eq!(result, Err(Ok(WardenError::RecoveryNotFound)));
}

// --- Phase 16 getters: get_account_state, get_guardians, get_recovery_proposal --

#[test]
fn get_account_state_returns_normal_by_default() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);

    let state = client.get_account_state(&wallet);
    assert_eq!(state, AccountState::Normal);
}

#[test]
fn get_account_state_reflects_a_written_state() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Watch);
    });

    let state = client.get_account_state(&wallet);
    assert_eq!(state, AccountState::Watch);
}

#[test]
fn get_guardians_returns_configured_guardians_and_threshold() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);

    client.set_guardians(&wallet, &addr_vec(&env, &[&g1, &g2]), &2u32);

    let config = client.get_guardians(&wallet);
    assert_eq!(config.guardians.len(), 2);
    assert!(config.guardians.contains(&g1));
    assert!(config.guardians.contains(&g2));
    assert_eq!(config.threshold, 2);
}

#[test]
fn get_guardians_fails_when_never_configured() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);

    let result = client.try_get_guardians(&wallet);
    assert_eq!(result, Err(Ok(WardenError::GuardiansNotConfigured)));
}

#[test]
fn get_recovery_proposal_returns_the_pending_proposal() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    client.set_guardians(&wallet, &addr_vec(&env, &[&g1, &g2]), &2u32);

    env.as_contract(&contract_id, || {
        storage::write_account_state(&env, &wallet, &AccountState::Restricted);
    });
    client.propose_recovery(&wallet, &g1, &AccountState::Normal);
    client.approve_recovery(&wallet, &g2);

    let proposal = client.get_recovery_proposal(&wallet);
    assert_eq!(proposal.proposer, g1);
    assert_eq!(proposal.target_state, AccountState::Normal);
    assert_eq!(proposal.approvals.len(), 2);
    assert_eq!(proposal.timelock_seconds, 172_800);
}

#[test]
fn get_recovery_proposal_fails_when_nothing_is_pending() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);

    let result = client.try_get_recovery_proposal(&wallet);
    assert_eq!(result, Err(Ok(WardenError::RecoveryNotFound)));
}
