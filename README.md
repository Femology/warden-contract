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

Full function-by-function reference, the event table, and the stated v1 limitation
(fixed velocity window, not sliding) are below in [Reference](#reference).

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
    pub new_recipient_requires_stepup: bool,
    pub trusted_recipients: Vec<Address>,
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
}
```

All amounts are `i128`. No monetary value in this contract, or anything built on top of
it, is ever represented as a float.

### Public functions

#### `initialize(admin: Address, reference_asset: Address)`
One-time setup at deploy. Requires `admin` auth. Fails with `AlreadyInitialized` if
called twice.

#### `set_policy(wallet: Address, max_no_stepup: i128, daily_velocity_cap: i128, new_recipient_requires_stepup: bool)`
Requires `wallet` auth. Creates a policy if none exists, or updates the three threshold
fields in place if one does — `trusted_recipients` is untouched by this call. Rejects with
`InvalidPolicyParams` if `max_no_stepup < 0` or `daily_velocity_cap < max_no_stepup`.
Emits `policy_set`.

#### `add_trusted_recipient(wallet: Address, recipient: Address)`
Requires `wallet` auth and an existing policy (`PolicyNotFound` otherwise). Fails with
`RecipientAlreadyTrusted` if already present. Emits `recipient_trusted`.

#### `remove_trusted_recipient(wallet: Address, recipient: Address)`
Requires `wallet` auth and an existing policy. Fails with `RecipientNotTrusted` if the
recipient isn't in the list. Emits `recipient_untrusted`.

#### `evaluate(wallet: Address, recipient: Address, amount: i128) -> Decision`
The core function, called at the moment a transfer is attempted. Requires `wallet` auth.

1. Loads the wallet's policy (`PolicyNotFound` if none is set — an unconfigured wallet
   gets an explicit error, not a silent default).
2. Validates `amount > 0` (`InvalidAmount` otherwise).
3. Loads (or starts fresh) the velocity window, resetting it if 24 hours have passed
   since `window_start`.
4. Decides, checking in this order and returning on first match: new untrusted recipient
   → step-up; amount over `max_no_stepup` → step-up; cumulative spend over
   `daily_velocity_cap` → step-up; otherwise → allow.
5. Updates the velocity window **regardless of the decision**. A transfer that triggered
   step-up and was then completed still happened and must count toward the cap —
   otherwise someone could reset their effective velocity limit just by making every
   transfer trigger step-up.
6. Emits `evaluation_allowed` or `stepup_required`.

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
| `policy_set` | `("policy_set", wallet)` | `(max_no_stepup, daily_velocity_cap, new_recipient_requires_stepup)` |
| `recipient_trusted` | `("recipient_trusted", wallet)` | `recipient` |
| `recipient_untrusted` | `("recipient_untrusted", wallet)` | `recipient` |
| `evaluation_allowed` | `("eval_allowed", wallet)` | `(recipient, amount)` |
| `stepup_required` | `("stepup_req", wallet)` | `(recipient, amount, reason)` |

These are built with soroban-sdk's `#[contractevent]` macro rather than the deprecated
`env.events().publish`, using explicit `topics` overrides and `data_format = "vec"` /
`"single-value"` so the on-chain wire shape matches this table exactly — a downstream
event indexer (`warden-monitor`) decodes against these literal topic names and positional
data, so they're treated as a stable interface, not an implementation detail.

### Known limitation: fixed velocity window, not a sliding one

The 24-hour window resets on expiry (`now - window_start >= 86400`) rather than sliding
continuously. A wallet could in principle spend up to its daily cap just before a reset,
then again just after — briefly doubling its effective velocity limit across that
boundary. This is a deliberate v1 simplification, not an oversight. A continuously
sliding window is a reasonable follow-up if this edge case matters for a given
deployment's risk tolerance.

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
