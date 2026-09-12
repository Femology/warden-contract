# Contributing to warden-contract

Thanks for looking at this. Warden is early — contributions of any size are welcome,
from a typo fix to a new test case to a real feature.

## Before you start

Read `../00-WARDEN-MASTER-PRD.md` (in the parent Warden planning repo, if you have
access) or this repo's own README for the project's non-negotiable rules. Read
[`WARDEN-PROTOCOL.md`](WARDEN-PROTOCOL.md) if your change touches `evaluate()`'s
decision logic or `AccountState` at all — see the next section. The short version,
specific to this contract:

- **The allow/step-up decision lives here and only here.** A change that would let the
  SDK, app, or monitor make or override that decision is out of scope for this repo.
- **No floats, ever.** Every amount is `i128`.
- **No `unwrap()` outside `#[cfg(test)]`.** Every fallible path returns a typed
  `WardenError`.
- **No placeholders or stubs.** Every commit should leave working, tested code.

## Changing a protocol rule

If your change touches **what triggers a state transition or a step-up reason** —
adding or changing a `StepUpReason` or `AccountState` value, changing `evaluate()`'s
check order, adding a new way `AccountState` can change, or changing what an error
code means — the ordinary "open an issue first" step below is not optional and has a
specific required shape. Open an issue using the [Protocol Rule Change
template](.github/ISSUE_TEMPLATE/protocol-rule-change.md) and get it acknowledged by a
maintainer *before* writing any code. See [`WARDEN-PROTOCOL.md`](WARDEN-PROTOCOL.md)'s
own Governance section for exactly why and the full required sequence — in short:
propose, get acknowledged, implement, update `WARDEN-PROTOCOL.md` in the same PR, add
a `CHANGELOG.md` entry. This is what keeps the protocol reviewable by someone who
didn't write it; skipping it for a rule change, however small, isn't a shortcut this
repo accepts.

A bug fix that makes the implementation match `WARDEN-PROTOCOL.md` more closely does
**not** need this process — the document is ground truth, and conforming to it isn't a
protocol change.

## Local setup

```bash
git clone https://github.com/Femology/warden-contract.git
cd warden-contract
rustup target add wasm32v1-none
cargo test
```

## Making a change

1. Open an issue first for anything beyond a trivial fix, so the approach can be agreed
   before you write code.
2. Branch from `main`.
3. One logical change per commit, with a clear commit message
   (`type(scope): description` -- `feat`, `fix`, `test`, `docs`, `chore`).
4. `cargo test` must pass locally before you open a PR.
5. Open a PR against `main`. CI (`cargo test`) must pass, and the PR needs one approval
   before it can merge.

## Reporting a bug

Open an issue with: what you expected, what happened instead, and the exact
`stellar contract invoke` command or test case that reproduces it. For a security issue,
see `SECURITY.md` instead -- don't open a public issue for that.

## Code style

- `snake_case` for functions and fields, `PascalCase` for types, `SCREAMING_SNAKE_CASE`
  for constants.
- Every persistent-storage write is paired with a TTL-extension call in the same
  function.
- Every mutating function's `require_auth()` call is its first line.
