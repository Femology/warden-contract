#![cfg(test)]

use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env};

use crate::{storage, Decision, StepUpReason, WardenContract, WardenContractClient, WardenError};

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
    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000);

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
    let result = client.try_set_policy(&wallet, &1_000, &5_000, &true, &6_000);
    assert_eq!(result, Err(Ok(WardenError::InvalidPolicyParams)));
}

#[test]
fn set_policy_updates_existing_policy_without_touching_trusted_recipients() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000);

    // Seed a trusted recipient directly through the storage layer, since
    // add_trusted_recipient does not exist yet at this point in the build
    // sequence (it lands in the next commit). This test only needs to prove
    // that set_policy leaves an existing trusted_recipients list untouched.
    env.as_contract(&contract_id, || {
        let mut policy = storage::read_policy(&env, &wallet).unwrap();
        policy.trusted_recipients.push_back(recipient.clone());
        storage::write_policy(&env, &wallet, &policy);
    });

    client.set_policy(&wallet, &2_000, &9_000, &false, &9_000);

    let policy = env
        .as_contract(&contract_id, || storage::read_policy(&env, &wallet))
        .expect("policy should exist after update");

    assert_eq!(policy.max_no_stepup, 2_000);
    assert_eq!(policy.daily_velocity_cap, 9_000);
    assert!(!policy.new_recipient_requires_stepup);
    assert_eq!(policy.trusted_recipients.len(), 1);
    assert_eq!(policy.trusted_recipients.get(0).unwrap(), recipient);
}

#[test]
fn set_policy_rejects_negative_max_no_stepup() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let result = client.try_set_policy(&wallet, &-1, &5_000, &true, &5_000);
    assert_eq!(result, Err(Ok(WardenError::InvalidPolicyParams)));
}

#[test]
fn set_policy_rejects_velocity_cap_below_max_no_stepup() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let result = client.try_set_policy(&wallet, &5_000, &1_000, &true, &1_000);
    assert_eq!(result, Err(Ok(WardenError::InvalidPolicyParams)));
}

#[test]
fn add_trusted_recipient_succeeds() {
    let (env, client, contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000);
    client.add_trusted_recipient(&wallet, &recipient);

    let policy = env
        .as_contract(&contract_id, || storage::read_policy(&env, &wallet))
        .unwrap();

    assert_eq!(policy.trusted_recipients.len(), 1);
    assert_eq!(policy.trusted_recipients.get(0).unwrap(), recipient);
}

#[test]
fn add_trusted_recipient_fails_when_already_trusted() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000);
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

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000);
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

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000);

    let result = client.try_remove_trusted_recipient(&wallet, &recipient);
    assert_eq!(result, Err(Ok(WardenError::RecipientNotTrusted)));
}

#[test]
fn set_policy_emits_policy_set_event_with_exact_topic_and_data_shape() {
    use soroban_sdk::{testutils::Events as _, vec as svec, IntoVal, Symbol, Val};

    let (env, client, contract_id, _admin, _reference_asset) = setup();
    let wallet = Address::generate(&env);

    client.set_policy(&wallet, &1_000i128, &5_000i128, &true, &5_000i128);

    let mut data: soroban_sdk::Vec<Val> = soroban_sdk::Vec::new(&env);
    data.push_back(1_000i128.into_val(&env));
    data.push_back(5_000i128.into_val(&env));
    data.push_back(5_000i128.into_val(&env));
    data.push_back(true.into_val(&env));

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

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000);

    let decision = client.evaluate(&wallet, &recipient, &500);
    assert_eq!(decision, Decision::Allow);
}

#[test]
fn evaluate_requires_stepup_for_new_recipient() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000);

    let decision = client.evaluate(&wallet, &recipient, &500);
    assert_eq!(decision, Decision::RequireStepUp(StepUpReason::NewRecipient));
}

#[test]
fn evaluate_requires_stepup_when_amount_exceeds_max() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000);

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

    client.set_policy(&wallet, &1_000, &1_500, &false, &1_500);

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

    client.set_policy(&wallet, &1_000, &1_000, &false, &1_000);

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

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000);

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

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000);

    let zero_result = client.try_evaluate(&wallet, &recipient, &0);
    assert_eq!(zero_result, Err(Ok(WardenError::InvalidAmount)));

    let negative_result = client.try_evaluate(&wallet, &recipient, &-5);
    assert_eq!(negative_result, Err(Ok(WardenError::InvalidAmount)));
}

#[test]
fn get_policy_returns_configured_policy() {
    let (env, client, _contract_id, _admin, _reference_asset) = setup();

    let wallet = Address::generate(&env);
    client.set_policy(&wallet, &1_000, &5_000, &true, &5_000);

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

    client.set_policy(&wallet, &1_000, &5_000, &false, &5_000);
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
    client.set_policy(&wallet, &1_000, &10_000, &false, &500);

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
