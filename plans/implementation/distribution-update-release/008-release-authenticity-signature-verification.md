# Distribution, Update, and Release Milestone 008 — Release Authenticity via Signature Verification

Status: proposed — blocked on ADR-0002 (signing scheme and key custody) and on an upstream Eggup manifest/signature contract; signing is currently an explicit roadmap non-goal

Repository baseline: `main` at `b4ec154` plus the uncommitted audit dependency refresh; re-verify at handoff

Source roadmap:

- `plans/subsystems/distribution-update-release-roadmap.md#7`
- `plans/subsystems/distribution-update-release-roadmap.md#10`
- `plans/subsystems/distribution-update-release-roadmap.md#3` (non-goals)

Long-term requirements:

- `plans/000-long-term-specification.md#3` (self-update exception)
- `plans/000-long-term-specification.md#6` (distribution and release)
- `plans/000-long-term-specification.md#7` (invariants)
- `plans/001-terminology-and-domain-model.md#7` (targets, transport, contract)

Applicable ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`
- `plans/adrs/ADR-0002` — **does not exist yet**; this plan's first work package creates it. Implementation must not begin before it is accepted.

Primary class: invariant + infrastructure

## 1. Objective

Make `eggsact update` verify a cryptographic signature over the release
metadata it already trusts, so that a compromised GitHub account/release or a
TLS interception cannot install arbitrary bytes that carry a matching
self-declared SHA-256.

Outcome: the updater refuses to stage any release whose manifest (or the
artifact set it describes) is not signed by the expected key, with no
digest-only fallback for releases covered by the policy.

## 2. Why this milestone is ready

It is **not** ready. Entry criteria:

- ADR-0002 defines the signature scheme, what bytes are signed, key custody,
  rotation, revocation, and the bootstrap/compatibility policy.
- The distribution roadmap's "no signing" non-goal is explicitly lifted for
  this scope.
- Eggup exposes a stable way to carry the signature (manifest field or a
  detached release asset), or a detached-asset contract is agreed and
  versioned.
- The release pipeline can produce the signature without hand-editing the
  generated workflow.

The local verification surface is otherwise stable: one updater module, one
manifest path, one legacy sidecar path.

## 3. Current implementation evidence

- `src/update.rs:946` compares digests in constant time. That is correct but
  only binds the downloaded bytes to a digest fetched from the same origin.
- `src/update.rs:1059-1070` fetches the release sidecar and rejects a
  mismatch; `src/update.rs:1134` passes the digest as Eggup's
  `IntegrityRequirement::Sha256`; `src/update.rs:1168` verifies integrity
  before staging.
- The trusted digest and the artifact both come from GitHub over the same
  TLS channel. A compromised release/account or a CA-level interception can
  supply malicious bytes plus a matching digest, which is then installed as
  the user's executable.
- `release/eggpack/*` is the producer authority; `.github/workflows/
  release-binaries.yml` is generated and must not be hand-edited.
- A repository-wide search found no `minisign`, `sigstore`, `cosign`, or
  signature references in `docs/release.md`, `architecture/`, `release/`, or
  `scripts/`; there is no existing signing contract to preserve.
- `plans/subsystems/distribution-update-release-roadmap.md#3` currently lists
  signing as a non-goal, so this milestone cannot be executed without an
  explicit roadmap/ADR decision.

## 4. Invariants that must not regress

- No network API is added to the library or MCP surface.
- Transport policy is unchanged: HTTP/1, rustls, strict HTTPS-downgrade
  rejection, bounded bodies, no retries.
- Verification happens before staging or committing a candidate; a failed
  signature never leaves a staged executable behind.
- Private keys never enter the repository, build outputs, logs, or test
  fixtures. Test keys are clearly marked and distinct.
- Release determinism flags and byte-reproducibility behavior are preserved;
  a detached signature is an additive asset and must not change artifact
  bytes.
- Existing version ordering, exact-asset selection, and `--version`
  candidate validation remain in force.
- A signature failure is a hard failure, not a warning.

## 5. Scope

### In scope

- ADR-0002: threat model, signed payload, scheme, key custody/rotation,
  bootstrap, and fail-closed policy.
- Producer-side signing through the Eggpack configuration and generated
  workflow, including how the signing key is supplied in CI.
- Updater-side verification with typed errors and bounded parsing.
- Test keys, tampered fixtures, and negative verification tests.
- Documentation of operator-visible failure modes.

### Explicitly out of scope

- Signing installers or other channels beyond the self-update path.
- Key escrow, HSM procurement, or organizational PKI.
- Changing the transport stack, adding retries, or adding a network API.
- MCP tool, registry, schema, or profile changes.
- Any change to `eggsact integrate`.

## 6. Required production changes

### Operator surface (CLI, update, integrate)

- Fetch the signature alongside the manifest with a request-local size cap.
- Verify before parsing/using per-asset digests; reject on missing, invalid,
  unknown-key, or malformed signature.
- Emit a distinct machine-readable failure reason and an actionable operator
  message (do not suggest `--force`; there is none).
- Remove or gate the manifest-absent legacy fallback for versions covered by
  the signature policy, per ADR-0002.

### Core/service/adapter

No change outside `src/update.rs` and its tests.

### Registry, profile, audience, surface

None.

### Protocol and DTOs

The manifest signature representation is Eggup-owned. This milestone may add
an external DTO only if the detached-asset contract is chosen; it must remain
private to the updater.

### Runtime and concurrency

Verification is synchronous within the update flow and bounded; no long-lived
state, no background work.

### Documentation and static guards

- `docs/release.md`: signing/verification contract, key rotation, and
  bootstrap.
- `architecture/self-update.md`: verification position in the pipeline.
- `AGENTS.md`: release-gate gotcha and "never hand-edit the generated
  workflow" reminder for signing changes.

## 7. Ordered work packages

### Work package A — ADR-0002 (decision gate)

Intent: decide before building.

Required content:

- Threat model: compromised GitHub release/account, malicious maintainer
  action, TLS interception, mirror/proxy substitution.
- What is signed: canonical manifest bytes covering release id, targets,
  artifact names, sizes, and SHA-256 values; or each artifact individually.
  Prefer signing the manifest so one signature binds the whole artifact set.
- Scheme options:
  1. Detached Ed25519/minisign signature over the manifest, long-lived public
     key embedded in the binary, signing key in CI secret storage.
  2. Sigstore/cosign keyless with an offline-verifiable bundle pinned to the
     release workflow identity.
  3. An Eggup-native signed-manifest field.
  Record tradeoffs: offline verification, dependency size, key rotation,
  revocation, and CI complexity.
- Key custody: who holds the private key, how it is backed up, how rotation
  happens (dual-signing window), and how a compromised key is revoked.
- Bootstrap: old binaries do not verify; the first signed release must still
  be installable by them. New binaries require signatures unconditionally,
  with no downgrade to digest-only.
- Legacy manifest-absent path behavior under the new policy.

Acceptance: ADR accepted and linked from this plan and the roadmap. No code
before this exists.

### Work package B — Producer signing contract

Intent: produce a verifiable signature without hand-editing generated CI.

Required changes:

- Extend `release/eggpack/` configuration (or the post-manifest hook Eggpack
  supports) so signing runs after manifest generation and before draft
  staging.
- Regenerate `release-binaries.yml` from the Eggpack configuration; the
  drift guard must pass.
- Supply the signing key from CI secret storage; never echo it; scope it to
  the stage job only.
- Publish the detached signature (or signed manifest) as part of the draft.

Acceptance: a rehearsal draft contains a signature that verifies against the
published public key; regeneration is reproducible; no workflow hand edit.

### Work package C — Updater verification

Intent: make verification unavoidable.

Required changes:

- Fetch the signature with a hard size cap and the existing transport.
- Verify the exact bytes that are later parsed; do not re-serialize before
  verification.
- Map failures to distinct reasons (missing, malformed, wrong key, bad
  signature, unsupported scheme) without leaking key material.
- Delete the staging directory on any verification failure.
- Ensure the signature is checked before any candidate `--version` execution.

Acceptance: tampered manifest, tampered asset, missing signature, truncated
signature, and wrong-key fixtures all abort before staging; valid fixtures
proceed.

### Work package D — Compatibility and bootstrap verification

Intent: prove the transition is safe for real users.

Required changes:

- Document the first signed release and the minimum updater version that
  verifies it.
- Verify that an old updater can install the first signed release (it ignores
  the added asset), and that a new updater rejects unsigned releases.
- Confirm no downgrade/replay path bypasses verification through the legacy
  sidecar branch.

Acceptance: recorded transition matrix (old->signed, new->unsigned,
new->signed, new->tampered).

### Work package E — Documentation and closure

Record key fingerprints, rotation procedure, ADR link, rehearsal run, and
negative-test evidence.

## 8. Failure, cancellation, truncation, and contention semantics

- Missing/invalid signature: abort before staging; remove partial downloads;
  no fallback to digest-only for covered releases.
- Truncated or oversized signature: classified transport/validation error;
  never partially verified.
- Wrong key: distinct message; do not attempt the next key unless the ADR
  defines a rotation window with an explicit ordered key set.
- Cancellation during verification: same cleanup as any other update
  cancellation; no staged executable remains.
- Concurrent updates: existing locking/ownership rules from Eggup apply; this
  milestone adds no new shared state.
- Network failure fetching the signature: hard failure, not a skip.

## 9. Compatibility and migration

- Adding a signature asset is additive for old clients.
- New clients require signatures for releases at or above the policy floor;
  the floor is defined in ADR-0002.
- The legacy manifest-absent fallback must not become a bypass; either it is
  removed for new clients or it requires a signature over the sidecar as
  well.
- Key rotation is a dual-signature release, not a flag day.
- If signing cannot be added to the generated pipeline without a hand edit,
  the milestone remains blocked rather than accepting a non-reproducible
  release step.

## 10. Required tests

### Focused unit tests

- Signature verification success/failure with test keys.
- Canonical-byte stability: verification uses the received bytes, not a
  re-encoded document.
- Distinct error classification for missing/malformed/wrong-key/bad-signature.

### Integration tests

- Fixture release with a valid signature installs.
- Tampered manifest and tampered asset are rejected before staging.
- Missing signature is rejected when the policy applies.

### Restart and recovery tests

- Failure mid-download and mid-verification leaves no staged candidate and a
  retry succeeds with a valid release.

### Contention and cancellation tests

- Cancellation during signature fetch/verify cleans up staging.
- Two concurrent updates keep existing Eggup ownership/lock behavior.

### Negative and boundary tests

- Zero-length, oversized, non-UTF-8, and truncated signature inputs.
- Wrong key, rotated key without dual signature, and a signature over a
  different manifest revision.
- Legacy path cannot bypass verification.

### Parity and compatibility tests

No tool behavior change; updater tests are the contract. Record that parity
was not run if unchanged.

## 11. Required verification commands

```bash
cargo test --locked --test lib update -- --test-threads=4
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
python3 scripts/check-release-contract.py
cargo deny check advisories bans licenses sources
```

Plus a signed rehearsal release run and verification against the draft asset.
Do not claim a live release was performed.

## 12. Documentation updates

- `docs/release.md` — signing, key custody, rotation, bootstrap.
- `architecture/self-update.md` — verification order and failure classes.
- `AGENTS.md` — generated-workflow rule and release-gate note.
- Roadmap — lift the signing non-goal for this scope, or record the decision
  not to proceed.

## 13. Acceptance criteria

- ADR-0002 is accepted and linked.
- A signed release verifies end-to-end in a rehearsal draft.
- Unsigned, tampered, wrong-key, and truncated inputs abort before staging.
- No private key or key material appears in the repository or logs.
- Old-client install of the first signed release is verified; new-client
  rejection of unsigned releases is verified.
- Generated workflow is regenerated from Eggpack config with a green drift
  guard.

## 14. Stop conditions

Stop and report when:

- ADR-0002 is not accepted, or key custody/rotation is undecided;
- the roadmap non-goal is not explicitly lifted;
- Eggup has no stable signature carrier and an upstream change cannot be
  scoped;
- signing would require hand-editing the generated workflow or a
  non-reproducible step;
- the only viable design is a digest-only fallback for covered releases;
- scope expands into installer signing, PKI, or transport changes.

## 15. Closure evidence required

- ADR-0002 with the accepted decision.
- Key fingerprints and custody/rotation record (no secrets).
- Rehearsal draft with signature verification output.
- Negative-test evidence for all rejection classes.
- Old/new transition matrix.
- Regenerated workflow diff and drift-guard output.
- Documentation links.

## 16. Handoff notes

- Signing is currently a non-goal; do not implement ahead of the ADR.
- Never commit a private key, even a test key used in fixtures must be
  explicitly non-production and clearly labeled.
- Do not hand-edit `.github/workflows/release-binaries.yml`; change
  `release/eggpack/` and regenerate.
- Verification must bind the exact received manifest bytes; re-serializing
  before verification is a defect.
- Keep the transport stack and bounded-body policy untouched.
