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
}
