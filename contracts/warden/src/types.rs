use soroban_sdk::{contracttype, Address, Vec};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    Admin,
    ReferenceAsset,
    Policy(Address),
    Velocity(Address),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Policy {
    pub owner: Address,
    pub max_no_stepup: i128,
    pub daily_velocity_cap: i128,
    pub new_recipient_requires_stepup: bool,
    pub trusted_recipients: Vec<Address>,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VelocityWindow {
    pub window_start: u64,
    pub cumulative_amount: i128,
    pub tx_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum StepUpReason {
    AmountExceeded,
    NewRecipient,
    VelocityExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum Decision {
    Allow,
    RequireStepUp(StepUpReason),
}

use soroban_sdk::contractevent;

#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicySetEvent {
    #[topic]
    pub wallet: Address,
    pub max_no_stepup: i128,
    pub daily_velocity_cap: i128,
    pub new_recipient_requires_stepup: bool,
}

#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientTrustedEvent {
    #[topic]
    pub wallet: Address,
    pub recipient: Address,
}

#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientUntrustedEvent {
    #[topic]
    pub wallet: Address,
    pub recipient: Address,
}
