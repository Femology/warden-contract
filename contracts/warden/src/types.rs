use soroban_sdk::{contracttype, Address, Map, Vec};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    Admin,
    ReferenceAsset,
    Policy(Address),
    Velocity(Address),
    HourlyVelocity(Address),
    // Presence + `true` means flagged; absence means not flagged. Removing
    // the key entirely on unflag (rather than writing `false`) keeps "is
    // this address flagged" a plain storage lookup with no stale entries
    // left behind for addresses that were flagged once and later cleared.
    FlaggedAddress(Address),
    // Absence means AccountState::Normal -- every wallet starts Normal
    // without needing an explicit write, same reasoning as VelocityWindow
    // defaulting to zeroed when absent.
    AccountState(Address),
    GuardianConfig(Address),
    // At most one pending proposal per wallet; absence means none pending.
    RecoveryProposal(Address),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Policy {
    pub owner: Address,
    pub max_no_stepup: i128,
    pub daily_velocity_cap: i128,
    // A separate, smaller-scoped cap evaluated over a rolling-reset 1h
    // window alongside the existing 24h one. Must be <= daily_velocity_cap
    // (set_policy rejects otherwise) -- allowing more per hour than per day
    // would make this cap meaningless. This is what catches rapid-fire
    // spending that would otherwise only be caught once the full daily cap
    // is reached, however long that takes.
    pub hourly_velocity_cap: i128,
    pub new_recipient_requires_stepup: bool,
    // Address -> last_paid_at (ledger timestamp of the most recent transfer
    // evaluate() saw to this recipient, or the timestamp it was added if
    // never paid since). A Map, not a Vec<(Address, u64)>: this is accessed
    // by key on every evaluate() call ("is this recipient in here, and
    // when did they last get paid"), which is exactly Map's get/set/
    // contains_key/remove access pattern -- a Vec of tuples would need a
    // linear scan (first_index_of + manual field comparison) for the same
    // lookup, with no benefit since insertion order is never used anywhere.
    pub trusted_recipients: Map<Address, u64>,
    // How long a recipient stays "trusted" for the new_recipient_requires_stepup
    // check after their last_paid_at, in seconds. A recipient past this age
    // still counts as being in the list (add/remove behave the same either
    // way) but no longer skips the new-recipient step-up until paid again.
    pub trust_decay_seconds: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VelocityWindow {
    pub window_start: u64,
    pub cumulative_amount: i128,
    pub tx_count: u32,
}

// Ordered least to most restrictive -- derived Ord follows this declaration
// order exactly (no explicit discriminants needed), which is what
// set_guardians' "NORMAL or WATCH only" gate and propose_recovery's
// "target_state must be strictly less restrictive" check both rely on:
// `state > AccountState::Watch` means Restricted/Challenged/Frozen, and
// `target_state < current_state` means genuinely less restrictive, not
// just different. Phase 16 is what introduces this type -- nothing earlier
// in this contract has any notion of account state.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[contracttype]
pub enum AccountState {
    Normal,
    Watch,
    Restricted,
    Challenged,
    Frozen,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GuardianConfig {
    pub guardians: Vec<Address>, // max 7, enforced by set_guardians
    pub threshold: u32,          // 1 <= threshold <= guardians.len()
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RecoveryProposal {
    pub proposer: Address,
    pub target_state: AccountState,
    pub approvals: Vec<Address>,
    pub proposed_at: u64,
    pub timelock_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum StepUpReason {
    AmountExceeded,
    NewRecipient,
    VelocityExceeded,
    HourlyVelocityExceeded,
    FlaggedRecipient,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum Decision {
    Allow,
    RequireStepUp(StepUpReason),
}

use soroban_sdk::contractevent;

#[contractevent(topics = ["policy_set"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicySetEvent {
    #[topic]
    pub wallet: Address,
    pub max_no_stepup: i128,
    pub daily_velocity_cap: i128,
    pub hourly_velocity_cap: i128,
    pub new_recipient_requires_stepup: bool,
    pub trust_decay_seconds: u64,
}

#[contractevent(topics = ["recipient_trusted"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientTrustedEvent {
    #[topic]
    pub wallet: Address,
    pub recipient: Address,
}

#[contractevent(topics = ["recipient_untrusted"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientUntrustedEvent {
    #[topic]
    pub wallet: Address,
    pub recipient: Address,
}

#[contractevent(topics = ["eval_allowed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationAllowedEvent {
    #[topic]
    pub wallet: Address,
    pub recipient: Address,
    pub amount: i128,
}

#[contractevent(topics = ["stepup_req"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepupRequiredEvent {
    #[topic]
    pub wallet: Address,
    pub recipient: Address,
    pub amount: i128,
    pub reason: StepUpReason,
}

#[contractevent(topics = ["address_flagged"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressFlaggedEvent {
    #[topic]
    pub admin: Address,
    pub address: Address,
}

#[contractevent(topics = ["address_unflagged"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressUnflaggedEvent {
    #[topic]
    pub admin: Address,
    pub address: Address,
}

#[contractevent(topics = ["guardians_set"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardiansSetEvent {
    #[topic]
    pub wallet: Address,
    pub guardians: Vec<Address>,
    pub threshold: u32,
}

#[contractevent(topics = ["recovery_proposed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryProposedEvent {
    #[topic]
    pub wallet: Address,
    pub proposer: Address,
    pub target_state: AccountState,
}

#[contractevent(topics = ["recovery_approved"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryApprovedEvent {
    #[topic]
    pub wallet: Address,
    pub guardian: Address,
    pub approvals_count: u32,
}

#[contractevent(topics = ["recovery_executed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryExecutedEvent {
    #[topic]
    pub wallet: Address,
    pub target_state: AccountState,
}

#[contractevent(topics = ["recovery_cancelled"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryCancelledEvent {
    #[topic]
    pub wallet: Address,
    pub proposer: Address,
    pub target_state: AccountState,
}
