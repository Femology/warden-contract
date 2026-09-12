<div align="center">

# Warden — Contract

**The policy engine behind Warden: decides whether a Stellar smart-wallet transfer needs
step-up confirmation, on-chain, at the wallet's own trust boundary.**

[![CI](https://github.com/Femology/warden-contract/actions/workflows/ci.yml/badge.svg)](https://github.com/Femology/warden-contract/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Network](https://img.shields.io/badge/network-Stellar%20Testnet-7D00FF)](https://stellar.expert/explorer/testnet/contract/CD25U7GYDNB7XUBEEN3OKZK2LY62ANSUJJPQ6SF2Y6DHQ5SQ3F7LSVUF)

[Warden org](https://github.com/Femology) · [warden-sdk](https://github.com/Femology/warden-sdk) · [warden-app](https://github.com/Femology/warden-app) · [warden-monitor](https://github.com/Femology/warden-monitor) · [Discussions](https://github.com/Femology/warden-contract/discussions)

</div>

---

## What this is

Most wallet apps treat every payment the same: a $2 transfer to a saved recipient gets
the same friction as a $2,000 transfer to a stranger. Warden lets a wallet owner set
their own rules — under this amount, to people I've paid before, just let it through —
so friction shows up where the risk actually is, not on every single tap.

`warden-contract` is the one place that decision is made. It's a single-purpose Soroban
contract: it stores per-wallet policy rules, tracks per-wallet spending velocity, checks
recipients against a small admin-managed flagged-address registry, and returns `Allow`
or `RequireStepUp(reason)`. It never moves funds, never calls another contract, and
never talks to anything off-chain — and nothing downstream (the SDK, the app, the
monitoring dashboard) is permitted to make or override that decision.

**Why Stellar specifically:** Soroban smart wallets support multiple signers with
distinct roles evaluated inside the wallet's own `__check_auth`. That means this decision
can live on-chain, at the same trust boundary as the wallet itself, instead of in a
backend service someone has to separately trust and operate.

## Architecture

```
set_policy, add/remove_trusted_recipient  →  Policy(Address)     [persistent storage]
add/remove_flagged_address (admin only)   →  FlaggedAddress(Address) -> bool
set_guardians                             →  GuardianConfig(Address)
propose/approve/cancel_recovery           →  RecoveryProposal(Address)
execute_recovery (no auth at all)         →  AccountState(Address), clears proposal
evaluate(wallet, recipient, amount)       →  checks FlaggedAddress(recipient) first
                                              → reads Policy + VelocityWindow(Address)
                                              → Decision::Allow | RequireStepUp(reason)
                                              → always updates VelocityWindow
get_policy, get_velocity                  →  public reads, no auth
```

Full function-by-function reference, the event table, and the stated v1 limitations
(fixed, not sliding, velocity windows) are below in [Reference](#reference).

## Quickstart

```bash
git clone https://github.com/Femology/warden-contract.git
cd warden-contract
rustup target add wasm32v1-none
cargo test
```

Build the deployable wasm:

```bash
stellar contract build
```

## Maintainers

| Name | GitHub | Contact |
|---|---|---|
| Femology | [@Femology](https://github.com/Femology) | femimi1234@gmail.com |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Found a security issue? See
[SECURITY.md](SECURITY.md) instead of opening a public issue.

<a href="https://github.com/Femology/warden-contract/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=Femology/warden-contract" alt="Contributors" />
</a>

---

## Reference

### Data model

```rust
pub struct Policy {
    pub owner: Address,
    pub max_no_stepup: i128,               // per-tx ceiling before step-up is required
    pub daily_velocity_cap: i128,          // cumulative ceiling per rolling 24h window
    pub hourly_velocity_cap: i128,         // cumulative ceiling per rolling 1h window, <= daily_velocity_cap
    pub new_recipient_requires_stepup: bool,
    pub trusted_recipients: Map<Address, u64>, // recipient -> last_paid_at (ledger timestamp)
    pub trust_decay_seconds: u64,          // how long trust survives without a payment
    pub updated_at: u64,
}

pub struct VelocityWindow {
    pub window_start: u64,
    pub cumulative_amount: i128,
    pub tx_count: u32,
}

pub enum Decision {
    Allow,
    RequireStepUp(StepUpReason),
}

pub enum StepUpReason {
    AmountExceeded,
    NewRecipient,
    VelocityExceeded,
    HourlyVelocityExceeded,
    FlaggedRecipient,
}

pub enum AccountState {
    Normal,
    Watch,
    Restricted,
    Challenged,
    Frozen,
}

pub struct GuardianConfig {
    pub guardians: Vec<Address>, // max 7
    pub threshold: u32,          // 1 <= threshold <= guardians.len()
}

pub struct RecoveryProposal {
    pub proposer: Address,
    pub target_state: AccountState,
    pub approvals: Vec<Address>,
    pub proposed_at: u64,
    pub timelock_seconds: u64,
}
```

A recipient's flagged status is stored as a separate `FlaggedAddress(Address) -> bool`
entry, one per address, not a field on `Policy` — it's a single global registry the
admin manages, not something each wallet owner configures for themselves. See
[Phase 15: flagged-address registry](#phase-15-flagged-address-registry) below.

`AccountState`, `GuardianConfig`, and `RecoveryProposal` are each their own storage
entry per wallet (`AccountState(Address)`, `GuardianConfig(Address)`,
`RecoveryProposal(Address)`), not fields on `Policy` — account state and guardian
recovery are a separate lifecycle concern from spending configuration, and guardians
are deliberately unable to touch `Policy` at all. Variants of `AccountState` are
ordered least to most restrictive (`Normal < Watch < Restricted < Challenged <
Frozen`); this ordering is what `set_guardians`' state gate and
`propose_recovery`'s "strictly less restrictive" check both compare against. See
[Phase 16: guardian and recovery subsystem](#phase-16-guardian-and-recovery-subsystem)
below.

Two separate `VelocityWindow` values are tracked per wallet — one resetting every 24h
(`daily_velocity_cap`), one resetting every 1h (`hourly_velocity_cap`) — the same shape,
different storage keys and reset periods. See
[Phase 14: dual velocity windows and trust decay](#phase-14-dual-velocity-windows-and-trust-decay)
below.

All amounts are `i128`. No monetary value in this contract, or anything built on top of
it, is ever represented as a float.

### Public functions

#### `initialize(admin: Address, reference_asset: Address)`
One-time setup at deploy. Requires `admin` auth. Fails with `AlreadyInitialized` if
called twice.

#### `set_policy(wallet: Address, max_no_stepup: i128, daily_velocity_cap: i128, new_recipient_requires_stepup: bool, hourly_velocity_cap: i128, trust_decay_seconds: u64)`
Requires `wallet` auth. Creates a policy if none exists, or updates these five
configurable fields in place if one does — `trusted_recipients` itself is untouched by
this call, only managed by the two functions below. Rejects with `InvalidPolicyParams`
if `max_no_stepup < 0`, `daily_velocity_cap < max_no_stepup`, `hourly_velocity_cap < 0`,
or `hourly_velocity_cap > daily_velocity_cap` (allowing more per hour than per day would
make the hourly cap meaningless). Emits `policy_set`.

#### `add_trusted_recipient(wallet: Address, recipient: Address)`
Requires `wallet` auth and an existing policy (`PolicyNotFound` otherwise). Fails with
`RecipientAlreadyTrusted` if already present. Sets `last_paid_at` to the current ledger
time — fresh, not decayed. Emits `recipient_trusted`.

#### `remove_trusted_recipient(wallet: Address, recipient: Address)`
Requires `wallet` auth and an existing policy. Fails with `RecipientNotTrusted` if the
recipient isn't in the list. Emits `recipient_untrusted`.

#### `add_flagged_address(admin: Address, address: Address)`
Requires `admin` auth, **and** that address must be the one stored at `initialize` —
`admin.require_auth()` alone only proves the caller really is that address, not that
the address is privileged, so this additionally checks it against the stored admin
(`NotAdmin` if it isn't a match, `NotInitialized` if no admin has been set yet at all).
Fails with `AddressAlreadyFlagged` if already flagged. Emits `address_flagged`. See
[Phase 15: flagged-address registry](#phase-15-flagged-address-registry) below for what
actually populates this in v1.

#### `remove_flagged_address(admin: Address, address: Address)`
Same admin gate as above. Fails with `AddressNotFlagged` if the address isn't
currently flagged. Emits `address_unflagged`.

#### `set_guardians(wallet: Address, guardians: Vec<Address>, threshold: u32)`
Requires `wallet` auth. Fails with `GuardianConfigLocked` if the account's current
state is worse than `Watch` (i.e. `Restricted`, `Challenged`, or `Frozen`) — stops an
attacker who just compromised a wallet from immediately adding their own colluding
guardian before the owner notices. Fails with `InvalidGuardianConfig` if
`guardians.len() > 7`, `threshold == 0`, or `threshold > guardians.len()`. Emits
`guardians_set`. See
[Phase 16: guardian and recovery subsystem](#phase-16-guardian-and-recovery-subsystem)
below.

#### `propose_recovery(wallet: Address, proposer: Address, target_state: AccountState)`
Requires `proposer` auth. Fails with `GuardiansNotConfigured` if `set_guardians` was
never called, `NotGuardian` if `proposer` isn't in the guardian list,
`RecoveryAlreadyProposed` if one is already pending, `InvalidTargetState` if
`target_state` isn't strictly less restrictive than the account's current state.
Starts the timelock at the current ledger time and counts the proposer's own approval
immediately — they don't call `approve_recovery` again for themselves. Emits
`recovery_proposed`.

#### `approve_recovery(wallet: Address, guardian: Address)`
Requires `guardian` auth. Fails with `GuardiansNotConfigured`, `NotGuardian` if
`guardian` isn't on the list, `RecoveryNotFound` if nothing is pending,
`AlreadyApproved` if this guardian already approved the current proposal. Emits
`recovery_approved`.

#### `execute_recovery(wallet: Address)`
**No auth requirement at all — not even the wallet's own.** This is the entire point
of guardian recovery: routing around a compromised or unavailable owner key. Callable
by anyone; only the stored state gates it. Fails with `GuardiansNotConfigured`,
`RecoveryNotFound` if nothing is pending, `InsufficientApprovals` if
`approvals.len() < threshold`, `TimelockNotElapsed` if
`now < proposed_at + timelock_seconds`. On success, transitions the account to
`target_state` and clears the proposal. Emits `recovery_executed`.

#### `cancel_recovery(wallet: Address)`
Requires `wallet` auth — the owner's veto, usable any time before `execute_recovery`
succeeds, protecting against a guardian-majority collusion attack while the owner's
own key is still fine. Fails with `RecoveryNotFound` if nothing is pending. Emits
`recovery_cancelled`.

#### `evaluate(wallet: Address, recipient: Address, amount: i128) -> Decision`
The core function, called at the moment a transfer is attempted. Requires `wallet` auth.

1. Loads the wallet's policy (`PolicyNotFound` if none is set — an unconfigured wallet
   gets an explicit error, not a silent default).
2. Validates `amount > 0` (`InvalidAmount` otherwise).
3. Loads (or starts fresh) both the daily and hourly velocity windows, resetting each
   independently if its own period has elapsed since its `window_start`.
4. Checks whether `recipient` counts as actively trusted: present in
   `trusted_recipients` **and** `now - last_paid_at <= trust_decay_seconds`. A recipient
   past that age is still in the list (unaffected by `add`/`remove_trusted_recipient`),
   it just no longer counts as trusted for this check.
5. Decides, checking in this order and returning on first match: `recipient` flagged in
   the global registry → step-up, **regardless of amount, trust, or velocity headroom**;
   new/decayed-untrusted recipient → step-up; amount over `max_no_stepup` → step-up;
   cumulative hourly spend over `hourly_velocity_cap` → step-up; cumulative daily spend
   over `daily_velocity_cap` → step-up; otherwise → allow. When both velocity windows
   are exceeded at once, the hourly reason is reported — it's the more specific, more
   immediately actionable signal.
6. Updates **both** velocity windows regardless of the decision, and refreshes
   `last_paid_at` for an already-trusted recipient regardless of the decision — same
   reasoning throughout: a transfer that triggered step-up and was then completed still
   happened, and must count. This does not touch `updated_at`, which means "the owner
   changed their policy configuration," not "a payment happened."
7. Emits `evaluation_allowed` or `stepup_required`.

The flagged-address check is evaluated after the policy is loaded (a wallet with no
policy still gets `PolicyNotFound`, flagged recipient or not) but ahead of every
policy-dependent check — it doesn't consult `trusted_recipients` or either velocity
window at all, since a flagged address's specific history with this wallet is beside
the point.

#### `get_policy(wallet: Address) -> Policy`
Public read, no auth. `PolicyNotFound` if none is set. Policy data isn't secret — it's
config, and everything on a public ledger is inspectable anyway.

#### `get_velocity(wallet: Address) -> VelocityWindow`
Public read, no auth. Unlike `get_policy`, this never errors on absence — a wallet with
no activity yet gets back a zeroed window, since "no activity" is a normal state for a
read, not a missing-configuration error.

### Events

| Event | Topics | Data |
|---|---|---|
| `policy_set` | `("policy_set", wallet)` | `(max_no_stepup, daily_velocity_cap, hourly_velocity_cap, new_recipient_requires_stepup, trust_decay_seconds)` |
| `recipient_trusted` | `("recipient_trusted", wallet)` | `recipient` |
| `recipient_untrusted` | `("recipient_untrusted", wallet)` | `recipient` |
| `address_flagged` | `("address_flagged", admin)` | `address` |
| `address_unflagged` | `("address_unflagged", admin)` | `address` |
| `guardians_set` | `("guardians_set", wallet)` | `(guardians, threshold)` |
| `recovery_proposed` | `("recovery_proposed", wallet)` | `(proposer, target_state)` |
| `recovery_approved` | `("recovery_approved", wallet)` | `(guardian, approvals_count)` |
| `recovery_executed` | `("recovery_executed", wallet)` | `target_state` |
| `recovery_cancelled` | `("recovery_cancelled", wallet)` | `(proposer, target_state)` |
| `evaluation_allowed` | `("eval_allowed", wallet)` | `(recipient, amount)` |
| `stepup_required` | `("stepup_req", wallet)` | `(recipient, amount, reason)` |

These are built with soroban-sdk's `#[contractevent]` macro rather than the deprecated
`env.events().publish`, using explicit `topics` overrides and `data_format = "vec"` /
`"single-value"` so the on-chain wire shape matches this table exactly — a downstream
event indexer (`warden-monitor`) decodes against these literal topic names and positional
data, so they're treated as a stable interface, not an implementation detail.

### Known limitation: fixed velocity windows, not sliding ones

Both the 24-hour and 1-hour windows reset on expiry (`now - window_start >= period`)
rather than sliding continuously. A wallet could in principle spend up to a cap just
before a reset, then again just after — briefly doubling its effective limit across
that boundary. This applies independently to each window. This is a deliberate v1
simplification, not an oversight. A continuously sliding window is a reasonable
follow-up if this edge case matters for a given deployment's risk tolerance.

### Phase 14: dual velocity windows and trust decay

Added after v0.1.0. Extends the same contract — see
[11-warden-feature-roadmap-phases-14-19.md](https://github.com/Femology/warden-planning/blob/main/11-warden-feature-roadmap-phases-14-19.md)
in `warden-planning` for the full rationale. Two things changed:

1. **A second, hourly velocity window** (`hourly_velocity_cap`, reset every 3600s)
   alongside the existing daily one, catching rapid-fire spending that a 24h cap alone
   wouldn't flag until far more had been spent. See `StepUpReason::HourlyVelocityExceeded`.
2. **Trust decay.** `trusted_recipients` now tracks *when* each recipient was last paid,
   not just whether they're trusted. A recipient who hasn't been paid in
   `trust_decay_seconds` no longer skips the new-recipient step-up, without needing to
   be explicitly removed and re-added.

#### Migration note — this was a breaking storage schema change

`Policy.trusted_recipients` changed type (`Vec<Address>` → `Map<Address, u64>`) and the
struct gained two new fields (`hourly_velocity_cap`, `trust_decay_seconds`). Soroban
contract storage decodes by the exact shape the currently-deployed wasm expects — **a
policy stored under the pre-Phase-14 contract cannot be read by this version**, and
there is no in-place upgrade path (this contract deliberately has no admin-upgrade
function, by design — see Scope below, and see the design discussion recorded in
`warden-planning`'s `DEPLOYMENT-INFO.md` on why an immutable contract was chosen over
an upgrade mechanism or a router/proxy pattern).

**What actually happened:** the v0.1.0 Testnet contract
(`CBFQ752LFNC57U4KWDAEKNU43PLBWJ7M2B4ZRYUMCWL62JHJNUYJVMB5`) — **deprecated, retired,
do not use** — held real policy data under the old schema from earlier testing. Rather
than an in-place upgrade (impossible, by design), Phase 14 was deployed as a genuinely
new contract instance:

| | |
|---|---|
| **Current contract ID** | [`CD25U7GYDNB7XUBEEN3OKZK2LY62ANSUJJPQ6SF2Y6DHQ5SQ3F7LSVUF`](https://stellar.expert/explorer/testnet/contract/CD25U7GYDNB7XUBEEN3OKZK2LY62ANSUJJPQ6SF2Y6DHQ5SQ3F7LSVUF) |
| **Deprecated (v0.1.0, pre-Phase-14)** | `CBFQ752LFNC57U4KWDAEKNU43PLBWJ7M2B4ZRYUMCWL62JHJNUYJVMB5` — retired, holds only stale pre-Phase-14 data, not reachable from current code |

The one wallet with real Testnet data called `set_policy` and `add_trusted_recipient`
again from scratch against the new contract — a wallet-signed action, not an admin
migration (`set_policy` requires the wallet's own `require_auth()`; nothing here or
anywhere in this contract can set a policy on another address's behalf). This was
acceptable for Testnet with no real funds at stake and no migration tooling built for
v1 — it would not be acceptable for a Mainnet deployment with real user data, which is
exactly the kind of gap a real migration tool or a deliberately-governed upgrade
mechanism would need to close before this contract is used for anything beyond
Testnet.

### Phase 15: flagged-address registry

Added after Phase 14. A single global registry of addresses the admin has flagged —
`evaluate()` checks the recipient against it before any policy-dependent check, and a
flagged recipient always returns `RequireStepUp(FlaggedRecipient)`, regardless of
amount, trust status, or velocity headroom.

#### What populates this registry in v1 — stated plainly

**It starts empty, and nothing populates it automatically.** `add_flagged_address` and
`remove_flagged_address` are manually called by the admin, one address at a time. There
is no external sanctions list, scam-address feed, chain-analytics API, or any other
automated data source wired up in v1 — this is infrastructure for a future real feed,
not a working integration with one yet. If and when a real source is chosen (for
example, a maintained list of addresses reported for fraud, or a chain-analytics
provider's flagged-address API), that choice and the sync mechanism belong in this
section, named honestly, when it actually exists.

**This is not an AI system, anywhere.** Flagging is a manual admin action against a
static on-chain list — there is no model making judgments about which addresses are
risky, here or in any component this contract talks to. Nothing in this contract,
`warden-sdk`, `warden-app`, or `warden-monitor` should ever be described as AI-driven
fraud detection; doing so would misrepresent what this registry actually is.

#### Why `admin`, not per-wallet

Unlike `trusted_recipients` (each wallet's own list, managed by that wallet), the
flagged-address registry is one shared list every wallet's `evaluate()` call checks
against — a wallet owner has no say over whether an address they're sending to is
flagged, by design. `admin.require_auth()` alone isn't sufficient to gate this: it
proves the caller really is whichever address they claim to be, not that the address is
privileged. `add_flagged_address`/`remove_flagged_address` additionally check the
caller against the address stored at `initialize`, failing with `NotAdmin` if it
doesn't match — the first genuinely privileged (non-self-authorizing) check in this
contract.

### Phase 16: guardian and recovery subsystem

Added after Phase 15. Lets a wallet owner name a set of guardians who can, together and
only after a timelock, move the account back toward normal operation if the owner's own
key is compromised or unavailable — without ever gaining any say over how the wallet
actually behaves once recovered.

#### Design decisions, stated explicitly

- **Guardians can only be configured while the account is `Normal` or `Watch`** — not
  `Restricted`, `Challenged`, or `Frozen`. Stops an attacker who just compromised a
  wallet from immediately adding their own colluding guardian before the owner notices.
- **`execute_recovery` does not require the wallet owner's own signature, or anyone
  else's.** This is the entire point of guardians — routing around a compromised owner
  key. Guardian threshold + elapsed timelock is sufficient; the function has no
  `require_auth` call on any address at all.
- **The owner can cancel a pending proposal at any time before it executes**, with
  their own signature. Protects against guardian-majority collusion in the ordinary
  case where the owner's key is fine and a recovery was proposed maliciously or in
  error.
- **Guardians can only ever move the account toward less restriction**, and only
  through this proposal/approval/timelock path — never instantly, never by simple
  majority alone, and never in the more-restrictive direction. (Nothing in this phase
  moves an account *into* a more restrictive state at all — see the open gap below.)
- **Guardians cannot touch `Policy` or `trusted_recipients` — full stop.** No function
  in this phase accepts a spending-limit or trusted-recipient argument, and none of the
  five new functions call `storage::write_policy` anywhere. Guardians recover access to
  normal operation; they never gain the ability to configure how the wallet behaves
  once recovered.

#### An honest gap: nothing escalates state yet

This phase builds the *recovery* half of a state machine — moving an account back
toward `Normal` — but nothing in `warden-contract` today ever moves an account into
`Watch`/`Restricted`/`Challenged`/`Frozen` in the first place. `AccountState` defaults
to `Normal` for every wallet and nothing currently changes it upward. That escalation
path is out of scope for this phase — the roadmap's own Phase 17 (an oracle layer) is
where automatic escalation would come from, explicitly marked conditional and not
started. Until that or some other escalation mechanism exists, this subsystem is real,
tested infrastructure with no way to actually trigger it outside of a test directly
seeding `AccountState` in storage (which is exactly how this phase's own tests do it).

#### An honest gap: no public getters

The roadmap's function list for this phase is `set_guardians`, `propose_recovery`,
`approve_recovery`, `execute_recovery`, `cancel_recovery` — five state-changing
functions, no reads. Built exactly as specified, which means **there is currently no
way for `warden-sdk`, `warden-app`, or `warden-monitor` to read a wallet's
`AccountState`, `GuardianConfig`, or pending `RecoveryProposal` via a contract call.**
`get_policy`/`get_velocity` exist as public reads for Phase 8's data; nothing analogous
was specified here. This is flagged deliberately, not fixed unasked: a
Guardians/Recovery Center UI cannot be built against this contract as it stands today
without either adding getters (a natural, small follow-up) or reconstructing state
entirely from the event log. Confirm which before starting frontend work.

#### An honest deviation: `timelock_seconds` is a constant, not a parameter

`RecoveryProposal.timelock_seconds` exists as a field (per spec), but neither
`set_guardians` nor `propose_recovery`'s given signature has a parameter to set it —
the roadmap's own comment calls it "configurable per wallet" without saying through
what function. Every proposal uses a fixed contract-wide constant,
`RECOVERY_TIMELOCK_SECONDS = 172800` (48 hours, the roadmap's own example value).
Deliberately not made a `propose_recovery` parameter: that would let the *proposer* — a
guardian, potentially a colluding one — choose their own timelock, undermining the
entire mechanism the timelock exists for (giving the owner a real window to notice and
cancel). A real "configurable per wallet" implementation would need the value set by
the owner (e.g., an added parameter on `set_guardians`), not the guardian proposing
recovery. Not built here since it isn't part of the given function signatures — stated
as a disclosed gap, not silently resolved either way.

### Tech stack

- **soroban-sdk**: pinned to exact `26.1.0` (not a caret range, and not the `27.0.0-rc`
  pre-release).
- **Rust**: edition 2021, MSRV 1.91.0 (matches soroban-sdk 26.1.0's own workspace
  manifest).
- **Wasm target**: `wasm32v1-none` — the only target the Soroban runtime supports on
  Rust ≥1.84; `wasm32-unknown-unknown` is not usable on Rust ≥1.82, since it enables
  wasm features (reference-types, multi-value) the runtime rejects.
- **Stellar CLI**: v27.0.0 for build/test/deploy tooling.

### Scope

This contract does not: move funds, call other contracts, support multiple assets,
register itself as a smart-wallet signer, or let anyone but a wallet's own owner change
that wallet's policy. Each of these was considered and deliberately left out of v1.

The one exception is the flagged-address registry (Phase 15): a single admin address,
set once at `initialize` and never changeable afterward (there is no
`transfer_admin`-style function in v1), can flag or unflag any address, affecting
every wallet's `evaluate()` calls against it. This is a deliberate, narrow, and
disclosed exception to "only a wallet's own owner acts on that wallet" — it's the
contract's one privileged, non-self-authorizing action, and it exists specifically so
a known-bad address can be blocked network-wide without every individual wallet owner
having to know about and separately flag it themselves.

Phase 16's guardians are a second, differently-shaped exception: a wallet owner opts
into naming their own guardians (self-configured, unlike the admin above), and those
guardians can — together, past a threshold, past a timelock — change that specific
wallet's `AccountState` without the owner's signature. `execute_recovery` is the one
function in this entire contract with no `require_auth` call on any address at all.
What guardians categorically cannot do, by construction, not just by convention: read
or write `Policy`, touch `trusted_recipients`, or affect any wallet other than the one
that named them. See
[Phase 16: guardian and recovery subsystem](#phase-16-guardian-and-recovery-subsystem)
above for the full design, including two disclosed gaps (no escalation path yet, no
public getters yet) that the next phase of work needs to resolve before a real UI can
be built against this.
