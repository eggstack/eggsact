# Distribution, Update, and Release Milestone 005 — Eggpack Producer Adoption and Live Draft Qualification

Status: blocked / planned

Repository baseline: `174764c5c71130ec98fee18c445fcecb3e35eb25`

Source roadmap:

- `plans/subsystems/distribution-update-release-roadmap.md`

Mirrored Eggpack plan:

- `eggstack/eggpack: plans/implementation/ecosystem-adoption/001-eggsact-direct-release-adoption-and-live-draft-qualification.md`

Required Eggpack prerequisite plans:

- `plans/implementation/ci-release-orchestration/003d-consumer-release-composition-seam.md`
- `plans/implementation/build-qualification/005-deterministic-cross-tool-provisioning.md`

Primary class: corrective adoption / release infrastructure / operational qualification

## 1. Why this closed workstream reopens

The distribution/update/release workstream was correctly closed after binary distribution and Eggfetch/Eggup updater qualification.

Eggpack now provides a shared producer-side release system that can replace duplicated eggsact build/qualification/checksum/draft-staging authority.

Per this repository's governance, adopting that new shared producer is a new corrective implementation plan rather than a silent edit to the closed roadmap.

This milestone does not reopen self-update correctness. It changes producer release construction while preserving user-facing installer and updater policy.

## 2. Objective

Adopt Eggpack as eggsact's producer authority for:

- five-target binary construction;
- exact toolchain/build floors;
- candidate qualification;
- target/artifact naming;
- checksum sidecars;
- ReleaseManifest;
- generated exact-release installers;
- generated checked-in GitHub release workflow;
- GitHub draft staging.

Preserve eggsact ownership of:

- crates.io publication;
- tag ordering;
- public installer latest/version selection;
- Cargo fallback;
- installation destinations/PATH advice;
- updater release selection/fallback/Eggup transaction;
- MCP protocol smoke semantics;
- final human publication.

Use the first normal release after implementation as Eggpack's real M003b live-draft qualification.

## 3. Hard dependencies

Do not implement this plan until Eggpack closes:

1. CI M003d consumer composition/runtime-identity seam;
2. Build M005 deterministic cross-tool provisioning.

At handoff, pin the exact closing Eggpack implementation SHA.

If either upstream closure reports a medium-or-higher unresolved finding affecting eggsact, update/re-review this plan before implementation.

## 4. Current authority to replace

Current producer release authority is duplicated in:

- `.github/workflows/release-binaries.yml`;
- `scripts/check-release-contract.py`;
- target/asset constants and build shell embedded in the workflow;
- release-side checksum generation;
- draft release creation/upload using `gh release ... --clobber`.

Current workflow also owns:

- Linux Zig archive provisioning;
- cargo-zigbuild installation;
- architecture smoke;
- candidate version/help smoke;
- `scripts/smoke-mcp-binary.py`;
- PowerShell installer parser check.

After M005, producer facts move to Eggpack configuration/generated CI.

## 5. Product policy that MUST remain local

### Crates.io/tag order

The release sequence remains:

1. clean tree + release check;
2. manual `cargo publish --locked`;
3. confirm crates.io accepted/indexed the version;
4. create annotated `vX.Y.Z` tag at the exact source;
5. push tag;
6. deliberately run binary release assembly;
7. inspect draft;
8. human publishes.

No Eggpack workflow publishes crates.io or creates/moves a tag.

### Public bootstrap wrappers

`packaging/install.sh` and `packaging/install.ps1` remain eggsact-owned.

They continue to own:

- default latest selection;
- explicit version selection;
- unsupported-host Cargo fallback;
- exact asset 404 -> Cargo fallback;
- checksum/TLS/5xx/version failures as hard errors;
- install destination and PATH advice.

### Self-update

`src/update.rs` remains eggsact policy over Eggup acquisition/transaction.

Do not replace its crates.io version authority or 404-only fallback with Eggpack in M005.

## 6. Checked-in Eggpack configuration

Add `release/eggpack/` with files matching the final M003d/Build-M005 schemas.

Expected conceptual inventory:

- DistributionContract;
- PackConfig;
- build bindings;
- qualification bindings;
- consumer validator policy;
- installer-presentation policy;
- static GitHub draft template;
- static GitHub runner/tool policy;
- reusable workflow shape/plan.

No file may contain:

- future release tag;
- future Git SHA;
- future artifact digest/size;
- mutable Eggpack revision.

The Eggpack CLI revision itself is pinned immutably in provider policy after upstream closure.

## 7. Five-target parity

The Eggpack config must reproduce the current public matrix exactly:

### Linux x86-64

- `x86_64-unknown-linux-gnu`;
- asset `eggsact-x86_64-unknown-linux-gnu`;
- CargoZigbuild;
- Zig 0.14.1;
- cargo-zigbuild 0.23.3;
- glibc 2.17;
- native x86-64 smoke/qualification.

### Linux AArch64

- `aarch64-unknown-linux-gnu`;
- asset `eggsact-aarch64-unknown-linux-gnu`;
- CargoZigbuild;
- Zig 0.14.1;
- cargo-zigbuild 0.23.3;
- glibc 2.17;
- native `ubuntu-24.04-arm`-class runner;
- native AArch64 smoke/qualification.

### macOS

- x86-64 -> `eggsact-x86_64-apple-darwin`;
- AArch64 -> `eggsact-aarch64-apple-darwin`;
- native build/qualification on matching hosts.

### Windows

- x86-64 -> `eggsact-x86_64-pc-windows-msvc.exe`;
- native x86-64 build/qualification.

ARMv7 remains product-recognized Cargo fallback only and is absent from the Eggpack release target set.

## 8. Toolchain provisioning parity

Build M005 must render the current verified Linux toolchain semantics.

For Zig 0.14.1 proving evidence:

- x86-64 archive SHA-256:
  `24aeeec8af16c381934a6cd7d95c807a8cb2cf7df9fa40d359aa884195c4716c`;
- AArch64 archive SHA-256:
  `f7a654acc967864f7a050ddacfaa778c7504a0eca8d2b678839c21eea47c992b`.

No apt Zig.

No ambient/unversioned cargo-zigbuild.

The generated workflow must prove exact versions before product build.

## 9. Candidate qualification parity

Core Eggpack smoke proves the exact binary executes and reports expected identity.

M003d consumer validation invokes:

`scripts/smoke-mcp-binary.py <exact candidate>`

using the finite Python3 semantic interpreter mapping:

- Linux/macOS -> Python 3 executable selected by Eggpack's finite host mapping;
- Windows -> Python 3 through the qualified Windows mapping.

The script remains eggsact-owned and unchanged unless a defect is found.

Required target consumer-validation failure blocks aggregation/draft staging.

The migration is not qualified if the generated workflow merely builds but stops running the MCP handshake.

## 10. Installer asset presentation

Configure Eggpack M003d ProductWrappers mode.

Public assets:

- source `packaging/install.sh` staged as `install.sh`;
- source `packaging/install.ps1` staged as `install.ps1`.

Generated exact-release installer assets:

- `install-exact.sh`;
- `install-exact.ps1`.

The generated scripts are additive release evidence/utility and do not replace the public wrapper UX in M005.

Update docs only to mention the exact installers if doing so is useful and non-confusing; the recommended install path remains the public wrappers.

## 11. Reusable runtime release identity

Initial generated workflow uses manual dispatch with required exact existing tag.

No checked-in ReleasePlan contains a future source SHA.

At runtime Eggpack:

1. checks out the supplied tag;
2. resolves exact HEAD;
3. resolves the exact ReleasePlan from static PackConfig/contract;
4. uses the exact tag as opaque release_id;
5. resolves the exact GitHub draft policy from the static template;
6. propagates those runtime documents to all jobs;
7. verifies each source checkout matches ReleasePlan.source_revision.

Any mismatch fails before release output.

## 12. Workflow cutover

### A. Generate and compare before replacement

Generate Eggpack release workflow to a temporary path.

Compare against current `.github/workflows/release-binaries.yml` for:

- target set;
- host architecture;
- cross tools;
- glibc floor;
- asset names;
- checksum sidecars;
- version/help execution;
- MCP handshake;
- Windows installer/source parsing where still relevant;
- permissions;
- draft-only behavior;
- tag/source verification.

### B. Replace active workflow only after parity

After all local/static parity checks pass:

- replace the active `.github/workflows/release-binaries.yml` with the generated workflow;
- do not leave a second active legacy release workflow;
- retain old implementation in Git history and closure evidence.

Initial Eggpack workflow is `workflow_dispatch` only.

### C. No legacy clobber path

The new workflow must contain no:

- `gh release upload --clobber`;
- arbitrary release deletion;
- publish command;
- tag creation/move.

## 13. Release-contract script refactor

Refactor `scripts/check-release-contract.py`.

Remove hand-maintained workflow assertions that duplicate Eggpack producer authority, including:

- target strings existing directly in YAML;
- asset strings existing directly in YAML;
- Zig shell fragments;
- hand-written release upload implementation.

Retain eggsact-owned invariants:

- Unix/PowerShell public wrapper fallback behavior;
- ARMv7 fallback-only recognition;
- updater no external curl;
- updater exact published target names;
- Eggup dependencies;
- generated workflow drift guard;
- one write-authorized staging job;
- no automated publication/tagging.

Where possible, compare product mappings against a checked-in projection produced from Eggpack config rather than another handwritten target list.

## 14. Merge/release drift guard

Add a CI check against the exact pinned Eggpack revision:

`eggpack ci check ...`

The guard must fail if the checked-in release workflow differs from static Eggpack configuration.

Provision Eggpack immutably; do not install from floating main.

Cache may improve speed but must not change correctness or pin verification.

Expose the same check from release-check documentation/scripts when practical.

## 15. Documentation changes

Update at minimum:

- `docs/release.md`;
- `docs/installation.md`;
- root README if release implementation details are described;
- distribution/update/release roadmap;
- planning registry.

Release docs must state:

- Eggpack is producer authority;
- crates.io publish remains manual first;
- tag remains manual after successful publish;
- binary release workflow is deliberately dispatched with the exact tag for initial adoption;
- draft publication remains human;
- public wrappers remain eggsact-owned;
- `eggsact update` remains eggsact/Eggup-owned.

## 16. Live real-draft qualification

Use the first normal release after cutover.

Do not create a fake public version merely for testing.

Required evidence:

1. crates.io version published successfully;
2. exact annotated tag exists at verified source;
3. generated Eggpack workflow manually dispatched with that tag;
4. all five target jobs pass;
5. all core qualification passes;
6. eggsact MCP consumer validation passes on exact candidates;
7. aggregate/finalization passes;
8. draft GitHub Release created/reused by Eggpack;
9. exact staged inventory is present;
10. release remains draft;
11. exact same workflow/tag rerun succeeds;
12. rerun reuses exact draft/assets without clobber.

Expected draft assets:

- five binaries;
- five checksum sidecars;
- `release-manifest.json`;
- `install.sh`;
- `install.ps1`;
- `install-exact.sh`;
- `install-exact.ps1`.

Record Eggpack staging receipt from both first run and rerun.

## 17. Human publication and post-release

Eggpack must not publish.

When maintainer elects to publish the validated draft:

- verify exact-tag public assets;
- verify latest public `install.sh` / `install.ps1`;
- verify pinned-version wrapper install path;
- verify latest-version wrapper path where practical;
- verify unsupported/404 Cargo fallback remains correct;
- verify `eggsact update` sees/handles the new stable version.

If publication is deliberately deferred, close M005 conditionally and record the outstanding public/latest smoke. Do not falsify publication evidence.

## 18. Updater compatibility

Do not delete `RELEASE_TARGETS` / target-selection policy from `src/update.rs` merely because Eggpack has a contract.

Runtime updater must remain self-contained.

Add static parity coverage so its prebuilt target names cannot drift from the Eggpack release contract.

A later Eggup interoperability milestone may reduce that duplication using a stable consumer manifest adapter.

## 19. Rollback

Before the first public release under Eggpack, rollback is a Git revert to the predecessor release workflow/config state.

After a successful public Eggpack-backed release, do not silently revert release authority without a corrective plan because the published asset set now includes Eggpack manifest/exact-installer evidence.

Public published versions/tags are immutable and never rewritten.

## 20. Verification

Minimum local gate:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
python3 scripts/check-release-contract.py
bash -n packaging/install.sh
scripts/release-check.sh
```

Plus:

- exact pinned Eggpack `ci check`;
- generated workflow parse/permission audit;
- installer PowerShell parse check;
- updater/Eggpack target parity tests;
- ordinary eggsact CI;
- MSRV 1.89;
- cargo-deny;
- native macOS/Windows maintenance lanes;
- real release workflow matrix at operational qualification.

## 21. Acceptance criteria

M005 implementation is complete only when:

- upstream M003d and Build M005 are closed/pinned;
- Eggpack static release config is checked in;
- generated workflow replaces the handwritten release matrix;
- workflow contains no future release-specific SHA/tag;
- five target/artifact pairs are unchanged;
- exact Zig/cargo-zigbuild/glibc policy is preserved;
- exact candidate MCP handshake remains gating;
- product wrappers preserve latest/version/Cargo fallback UX;
- updater policy remains unchanged except parity guards;
- release-contract script no longer duplicates producer implementation details;
- `eggpack ci check` gates workflow drift;
- ordinary/maintenance CI passes;
- real draft qualification and rerun pass when an authorized release tag exists;
- Eggpack never publishes the draft.

Full closure additionally records post-publication public/latest installer smoke if the maintainer publishes during the closure window.

## 22. Stop conditions

Stop and re-plan if:

- upstream Eggpack M003d/M005 changes invalidate this interface;
- generated workflow cannot preserve all five target/runner requirements;
- public wrapper behavior must be weakened;
- exact MCP candidate validation cannot gate draft creation;
- updater policy must migrate to complete the cutover;
- crates.io/tag authority would move into CI;
- live staging needs release asset clobbering or tag mutation;
- legacy and Eggpack release workflows would both trigger for the same release.

## 23. Closure evidence

Create:

`plans/closure/distribution-update-release/005-status.md`

Record:

- Eggpack pinned SHA;
- eggsact implementation SHA;
- predecessor vs generated workflow authority map;
- static config inventory;
- five-target parity matrix;
- Linux tool provisioning evidence;
- exact candidate MCP validation matrix;
- wrapper semantic parity;
- updater parity;
- local/ordinary/maintenance CI runs;
- live draft run/tag;
- first staging receipt;
- rerun staging receipt;
- exact draft asset inventory;
- confirmation Eggpack left release unpublished;
- post-publication evidence or explicit outstanding condition;
- unresolved findings;
- Eggpack CI M003b/Phase 8 closure disposition.
