# Changelog

All notable changes to **the Warden protocol** — the rules specified in
[`WARDEN-PROTOCOL.md`](WARDEN-PROTOCOL.md) — are recorded here, in the format
established by [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioned
according to [Protocol versioning](WARDEN-PROTOCOL.md#protocol-versioning).

**This file tracks the protocol's version, not `warden-contract`'s release tags.** See
the repo's [GitHub Releases](https://github.com/Femology/warden-contract/releases) for
the implementation's own version history (currently
[v0.3.0](https://github.com/Femology/warden-contract/releases/tag/v0.3.0)) — the two
numbers are independent and will drift, by design.

Every entry below either originated from a [Protocol Rule Change
issue](.github/ISSUE_TEMPLATE/protocol-rule-change.md) (linked where one exists) or
predates the governance process itself (see the `1.0.0` entry).

## [1.0.0] - 2026-09-12

Initial protocol specification. Documents the rules as already implemented in
`warden-contract` through its v0.3.0 release — this version bump did not itself change
any rule; it's the first time those rules were written down independent of the Rust
source.

### Specified

- The five-check `evaluate()` decision order: flagged recipient, then new/decayed
  recipient, then amount, then hourly velocity, then daily velocity.
- Five `StepUpReason` values: `AmountExceeded`, `NewRecipient`, `VelocityExceeded`,
  `HourlyVelocityExceeded`, `FlaggedRecipient`.
- The five-state `AccountState` ordering (`Normal < Watch < Restricted < Challenged <
  Frozen`) and the single legal transition mechanism that exists: guardian recovery,
  always strictly de-escalating.
- The flagged-address registry (admin-managed, starts empty, no automated feed as of
  this version) and the guardian/recovery subsystem (owner-configured guardians,
  threshold- and timelock-gated de-escalation, no owner signature required to
  execute).
- The full event schema (12 events) and error code table (20 codes) as stable,
  consumer-facing interfaces.
- Explicit confirmation that Phase 17 (an oracle/attestation layer capable of
  automatic escalation) is not active, and that no signed-attestation schema exists
  yet.
- The governance process itself (this file, the issue template, and § Governance in
  `WARDEN-PROTOCOL.md`) for any future change to state-transition or step-up-reason
  triggers.
