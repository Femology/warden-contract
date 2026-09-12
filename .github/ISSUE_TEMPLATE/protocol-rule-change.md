---
name: Protocol rule change
about: Propose a change to what triggers a state transition or a step-up reason. Required before implementation -- see WARDEN-PROTOCOL.md's Governance section.
title: "[protocol] "
labels: protocol-change
---

<!--
Before filling this in: read WARDEN-PROTOCOL.md, specifically "The evaluation
decision" and "Account state diagram and every legal transition". This template
exists so a rule change is visible and arguable BEFORE it's a fait accompli in
main -- fill in all three required sections below, or this issue isn't ready for
implementation to start against.

Do NOT open this template for: a bug fix that makes the implementation match
WARDEN-PROTOCOL.md more closely (the document is ground truth; conforming to it
isn't a protocol change), a refactor with no behavioral difference, or a change
scoped entirely to implementation detail (storage layout, build tooling, TTL
bump amounts). If you're not sure which this is, say so in the issue and a
maintainer will help you sort it out -- that's a normal, fine thing to ask.
-->

## What changes

<!--
State the exact rule change, precisely enough that someone could implement it
from this section alone. Reference the exact section(s) of WARDEN-PROTOCOL.md
this would edit -- "The evaluation decision", a specific row of the
StepUpReason or AccountState tables, the transition diagram, the event schema,
etc. If this adds a new StepUpReason, AccountState value, event, or error
code, say so explicitly and give its exact name and meaning.
-->

## Why

<!--
The concrete problem this solves or the concrete gap this closes. "Would be
nice" is not sufficient justification for a protocol change -- name the real
scenario (a real attack this prevents, a real false positive this fixes, a
real capability a real consumer needs) that motivates changing a rule
everything downstream currently relies on staying stable.
-->

## What it affects

<!--
Everyone and everything that has to change if this is accepted:
- Which functions' behavior changes, and how existing callers of them are
  affected (a behavior change for an existing input is a bigger deal than an
  additive one for a new input).
- Whether this is a protocol MAJOR or MINOR version bump, per
  WARDEN-PROTOCOL.md's "Protocol versioning" section, and why.
- Every downstream consumer that needs updating once this ships: warden-sdk
  (new/changed types, decode logic), warden-app and warden-monitor (any UI
  that names a StepUpReason or AccountState explicitly, e.g. an exhaustive
  Record<StepUpReason, ...> map -- TypeScript will catch these, but call out
  where you expect it to), warden-docs (any example whose output would change).
- Whether existing on-chain data (a live Policy, a live RecoveryProposal) is
  affected, and if so, whether that requires a new contract deployment the
  same way past storage-shape changes have (see WARDEN-PROTOCOL.md and
  warden-contract's README for precedent).
-->

## Acceptance

<!--
Leave this section for a maintainer. A protocol change is accepted only once a
maintainer confirms understanding here and states which version bump (MAJOR or
MINOR) it implies -- only then does implementation start. See
WARDEN-PROTOCOL.md's Governance section for the full required sequence after
this: implement, update WARDEN-PROTOCOL.md in the same PR, add a CHANGELOG.md
entry.
-->
