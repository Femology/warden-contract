#!/usr/bin/env bash
# Creates every planned next-step issue for warden-contract in one run.
# Run from inside the warden-contract repo with gh authenticated.
set -euo pipefail

gh issue create \
  --title "feat(contract): register warden-contract as a smart-wallet policy signer" \
  --label "enhancement,complexity: large" \
  --body "## Summary
Currently, the step-up gate is app-enforced: \`warden-app\` asks \`evaluate()\` for a
decision and its own UI refuses to proceed without confirmation, but nothing on-chain
stops a modified client from ignoring the answer. This issue is the real fix: register
\`warden-contract\` as a genuine policy signer inside a passkey-kit smart wallet's own
\`__check_auth\`, so step-up is cryptographically enforced, not just UI-enforced.

## Acceptance Criteria
- [ ] Verify passkey-kit's current multi-signer / policy-signer interface against its
      actual source (not the version assumed in earlier phases) -- this was
      deliberately left unverified in v1, see the contract README's stated limitation.
- [ ] Design how \`evaluate()\`'s result gates the wallet's own auth check.
- [ ] Implement and test the signer registration path.
- [ ] Update \`warden-app\`'s README to remove the app-enforced-gate limitation once
      this ships.

## Tech Stack
Rust, soroban-sdk 26.1.0, passkey-kit (version to be re-verified at implementation
time)."

gh issue create \
  --title "feat(contract): slide the velocity window instead of resetting on expiry" \
  --label "enhancement,complexity: medium" \
  --body "## Summary
The 24-hour velocity window currently resets on expiry (\`now - window_start >= 86400\`)
rather than sliding continuously. A wallet can spend up to its daily cap just before a
reset and again just after, briefly doubling its effective limit across that boundary.
This is a stated, deliberate v1 simplification -- this issue tracks fixing it for real.

## Acceptance Criteria
- [ ] Design a sliding-window accounting scheme that fits Soroban's storage model
      without unbounded per-transaction history.
- [ ] Implement and add tests specifically covering the reset-boundary edge case this
      replaces.
- [ ] Confirm gas/resource cost impact is acceptable versus the current fixed-window
      approach.

## Tech Stack
Rust, soroban-sdk 26.1.0."

gh issue create \
  --title "feat(contract): surface every simultaneously-true step-up reason, not just the first match" \
  --label "enhancement,complexity: small" \
  --body "## Summary
\`evaluate()\` currently returns only the first matching step-up reason (new recipient,
then amount, then velocity, in that order). A transfer could trigger more than one
reason at once; v1 doesn't need more than one, but a reviewer or integrator may want
full visibility into every reason that applied.

## Acceptance Criteria
- [ ] Decide the return shape: a \`Vec<StepUpReason>\` on \`RequireStepUp\`, or a
      dedicated read-only helper that reports all applicable reasons without changing
      \`evaluate()\`'s existing return type (a breaking-change consideration for
      \`warden-sdk\`/\`warden-app\`).
- [ ] Implement, with tests for every combination of simultaneously-true reasons.
- [ ] Update \`warden-sdk\`'s \`Decision\`/\`StepUpReason\` types and \`warden-app\`'s modal
      copy if the shape changes.

## Tech Stack
Rust, soroban-sdk 26.1.0. Touches \`warden-sdk\` and \`warden-app\` if the return shape
changes."

gh issue create \
  --title "chore(contract): commission a third-party security audit before any Mainnet deployment" \
  --label "type: security,complexity: large" \
  --body "## Summary
\`warden-contract\` has not had an independent security audit (stated plainly in
SECURITY.md). This is the actual blocker before any Mainnet deployment with real funds
at stake -- everything else in this repo assumes Testnet only.

## Acceptance Criteria
- [ ] Identify and budget for a Soroban-experienced audit firm.
- [ ] Freeze the contract's function surface before audit engagement (no new public
      functions mid-audit).
- [ ] Address every finding, with a public summary of what was found and fixed.
- [ ] Update SECURITY.md once an audit has actually happened -- it currently states
      \"unaudited\" as a fact, not a placeholder; keep it honest going forward too.

## Tech Stack
N/A -- process and vendor engagement, not code."

echo "Done. Created 4 issues."
