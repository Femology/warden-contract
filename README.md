<div align="center">

# Warden — Contract

**The policy engine behind Warden: decides whether a Stellar smart-wallet transfer needs
step-up confirmation, on-chain, at the wallet's own trust boundary.**

[![CI](https://github.com/Femology/warden-contract/actions/workflows/ci.yml/badge.svg)](https://github.com/Femology/warden-contract/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Network](https://img.shields.io/badge/network-Stellar%20Testnet-7D00FF)](https://stellar.expert/explorer/testnet/contract/CBFQ752LFNC57U4KWDAEKNU43PLBWJ7M2B4ZRYUMCWL62JHJNUYJVMB5)

[Warden org](https://github.com/Femology) · [warden-sdk](https://github.com/Femology/warden-sdk) · [warden-app](https://github.com/Femology/warden-app) · [warden-monitor](https://github.com/Femology/warden-monitor) · [Discussions](https://github.com/Femology/warden-contract/discussions)

</div>

---

## What this is

Most wallet apps treat every payment the same: a $2 transfer to a saved recipient gets
the same friction as a $2,000 transfer to a stranger. Warden lets a wallet owner set
their own rules — under this amount, to people I've paid before, just let it through —
so friction shows up where the risk actually is, not on every single tap.

`warden-contract` is the one place that decision is made. It's a single-purpose Soroban
contract: it stores per-wallet policy rules, tracks per-wallet spending velocity, and
returns `Allow` or `RequireStepUp(reason)`. It never moves funds, never calls another
contract, and never talks to anything off-chain — and nothing downstream (the SDK, the
app, the monitoring dashboard) is permitted to make or override that decision.

**Why Stellar specifically:** Soroban smart wallets support multiple signers with
distinct roles evaluated inside the wallet's own `__check_auth`. That means this decision
can live on-chain, at the same trust boundary as the wallet itself, instead of in a
backend service someone has to separately trust and operate.

## Architecture

```
set_policy, add/remove_trusted_recipient  →  Policy(Address)     [persistent storage]
evaluate(wallet, recipient, amount)       →  reads Policy + VelocityWindow(Address)
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
}
```

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
5. Decides, checking in this order and returning on first match: new/decayed-untrusted
   recipient → step-up; amount over `max_no_stepup` → step-up; cumulative hourly spend
   over `hourly_velocity_cap` → step-up; cumulative daily spend over
   `daily_velocity_cap` → step-up; otherwise → allow. When both velocity windows are
   exceeded at once, the hourly reason is reported — it's the more specific, more
   immediately actionable signal.
6. Updates **both** velocity windows regardless of the decision, and refreshes
   `last_paid_at` for an already-trusted recipient regardless of the decision — same
   reasoning throughout: a transfer that triggered step-up and was then completed still
   happened, and must count. This does not touch `updated_at`, which means "the owner
   changed their policy configuration," not "a payment happened."
7. Emits `evaluation_allowed` or `stepup_required`.

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

#### Migration note — this is a breaking storage schema change

`Policy.trusted_recipients` changed type (`Vec<Address>` → `Map<Address, u64>`) and the
struct gained two new fields (`hourly_velocity_cap`, `trust_decay_seconds`). Soroban
contract storage decodes by the exact shape the currently-deployed wasm expects — **a
policy stored under the pre-Phase-14 contract cannot be read by this version**, and
there is no in-place upgrade path (this contract deliberately has no admin-upgrade
function, by design — see Scope below).

Concretely: `warden-contract`'s v0.1.0 Testnet deployment
(`CBFQ752LFNC57U4KWDAEKNU43PLBWJ7M2B4ZRYUMCWL62JHJNUYJVMB5`) already has real policy
data under the old schema (from earlier testing). Deploying this Phase 14 code means a
**new contract instance, a new contract ID** — not an upgrade of the existing one.
Every wallet, including that Testnet one, needs to call `set_policy` again from
scratch under the new deployment; nothing carries over automatically. This is
acceptable for Testnet with no real funds at stake and no migration tooling built for
v1 — it would not be acceptable for a Mainnet deployment with real user data, which is
exactly the kind of gap a real migration tool or an upgrade mechanism would need to
close before this contract is used for anything beyond Testnet.

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
