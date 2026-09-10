# Security Policy

## Unaudited -- use at your own risk

**`warden-contract` has not had a third-party security audit.** It has been reviewed
against its own test suite (24 tests, covering every function and error path) and
deployed to Stellar Testnet, but no independent security firm has reviewed this code. Do
not deploy this contract on Mainnet with real funds at stake without a proper audit
first. This is stated here plainly, not as boilerplate -- it is the actual current state
of this project.

## Reporting a vulnerability

If you find a security issue in this contract -- anything that could let a wallet's
policy or velocity state be corrupted, bypassed, or read/written by an address other
than the wallet's own owner -- please report it privately rather than opening a public
issue.

**Contact:** femimi1234@gmail.com

Include:
- A description of the issue and its impact.
- Steps to reproduce, ideally as a failing test case against this repo.
- Whether you believe it's exploitable on the current Testnet deployment.

You'll get an acknowledgment within a few days. Please give a reasonable amount of time
to address the issue before any public disclosure.

## Scope

In scope: the `warden-contract` Soroban contract itself (`contracts/warden/src/`). Out
of scope: the Stellar network/protocol itself, `soroban-sdk`, and downstream repos
(`warden-sdk`, `warden-app`, `warden-monitor`) -- report issues in those to their own
repos.
