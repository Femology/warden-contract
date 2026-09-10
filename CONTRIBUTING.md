# Contributing to warden-contract

Thanks for looking at this. Warden is early — contributions of any size are welcome,
from a typo fix to a new test case to a real feature.

## Before you start

Read `../00-WARDEN-MASTER-PRD.md` (in the parent Warden planning repo, if you have
access) or this repo's own README for the project's non-negotiable rules. The short
version, specific to this contract:

- **The allow/step-up decision lives here and only here.** A change that would let the
  SDK, app, or monitor make or override that decision is out of scope for this repo.
- **No floats, ever.** Every amount is `i128`.
- **No `unwrap()` outside `#[cfg(test)]`.** Every fallible path returns a typed
  `WardenError`.
- **No placeholders or stubs.** Every commit should leave working, tested code.

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
