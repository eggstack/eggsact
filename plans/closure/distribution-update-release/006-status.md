# Distribution, Update, and Release Milestone 006 — Closure Status

Status: closed

Source implementation plan: `plans/archive/implementation/distribution-update-release/006-dependency-provenance-and-source-policy.md`

Related milestone: M007 registry adoption, recorded in `plans/closure/distribution-update-release/007-status.md`.

Dependency update commit: `a8416e9868e419f72942bb42795f19d4cbc9c9a2` on `audit/security-followups-006-009`; [PR #1](https://github.com/eggstack/eggsact/pull/1).

## Closure evidence

| Requirement | Evidence | Result |
|---|---|---|
| Cargo source policy remains strict | `deny.toml` has `allow-git = []`; Maintenance `cargo-deny` passed in [run 45089096709](https://github.com/eggstack/eggsact/actions/runs/45089096709). | pass |
| Provenance preflight detects versionless Git dependencies and handles valid manifests | `python3 scripts/check-dependency-provenance.py --self-test` passed all 10 cases; production manifest passes. The archived implementation note records the negative second-source denial. | pass |
| Strict release qualification is clean-tree safe | `scripts/release-check.sh` passed on clean commit `a8416e9`. | pass |
| Registry adoption restores package qualification | M007 replaced the versionless Git pins with registry dependencies, removed the temporary allow-list, and recorded package/publish dry-run and ZIP 9.6.1 evidence in its closure record. | pass |

Maintenance run [45089096709](https://github.com/eggstack/eggsact/actions/runs/45089096709) passed all five jobs: macOS and Windows checks, `cargo-deny`, MSRV, and Windows reproducibility. The dependency update was not published or tagged; publication remains a separate maintainer action.
