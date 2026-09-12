use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum WardenError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    PolicyNotFound = 3,
    InvalidAmount = 4,
    InvalidPolicyParams = 5,
    RecipientAlreadyTrusted = 6,
    RecipientNotTrusted = 7,
    // The caller's require_auth() succeeded (they really are who they claim
    // to be) but that address isn't the one stored at initialize -- proves
    // identity, not privilege. Distinct from NotInitialized, which means no
    // admin has been set at all yet.
    NotAdmin = 8,
    AddressAlreadyFlagged = 9,
    AddressNotFlagged = 10,
    // guardians.len() > 7, threshold == 0, or threshold > guardians.len().
    InvalidGuardianConfig = 11,
    // set_guardians called while the account is RESTRICTED or worse (state
    // > AccountState::Watch) -- stops an attacker who just compromised a
    // wallet from immediately adding their own colluding guardian.
    GuardianConfigLocked = 12,
    // propose_recovery/approve_recovery/execute_recovery called on a wallet
    // that never called set_guardians.
    GuardiansNotConfigured = 13,
    // The caller's require_auth() succeeded but they aren't in the
    // wallet's guardian list.
    NotGuardian = 14,
    // propose_recovery's target_state is not strictly less restrictive than
    // the account's current state.
    InvalidTargetState = 15,
    // propose_recovery called while a proposal is already pending -- only
    // one at a time; cancel or execute the existing one first.
    RecoveryAlreadyProposed = 16,
    // approve_recovery/execute_recovery/cancel_recovery called with no
    // pending proposal.
    RecoveryNotFound = 17,
    // approve_recovery called twice by the same guardian for one proposal.
    AlreadyApproved = 18,
    // execute_recovery called before approvals.len() >= threshold.
    InsufficientApprovals = 19,
    // execute_recovery called before now >= proposed_at + timelock_seconds.
    TimelockNotElapsed = 20,
}
