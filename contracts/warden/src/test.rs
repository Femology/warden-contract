#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{WardenContract, WardenContractClient, WardenError};

fn setup<'a>() -> (Env, WardenContractClient<'a>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WardenContract, ());
    let client = WardenContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let reference_asset = Address::generate(&env);

    (env, client, admin, reference_asset)
}

#[test]
fn initialize_succeeds_on_first_call() {
    let (_env, client, admin, reference_asset) = setup();

    client.initialize(&admin, &reference_asset);
}

#[test]
fn initialize_fails_when_already_initialized() {
    let (_env, client, admin, reference_asset) = setup();

    client.initialize(&admin, &reference_asset);

    let result = client.try_initialize(&admin, &reference_asset);
    assert_eq!(result, Err(Ok(WardenError::AlreadyInitialized)));
}
