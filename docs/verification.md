# Verification Doctrine

This document defines the verification tiers for eggsact. Each tier has a different cadence, purpose, and ownership.

## Tier 1 — Ordinary Merge CI

Required on every pull request and push to `main`. Answers: "Is this change safe to merge?"

### Linux correctness

Runs on `ubuntu-latest` in a single job with one compilation cache:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

## Tier 2 — Scheduled Compatibility and Policy Checks

Run weekly (Monday) and via `workflow_dispatch`. Do not block ordinary merges. Answers: "Are external dependencies drifting?"

| Check | Workflow | Cadence |
|-------|----------|---------|
| MSRV compilation and library tests | `maintenance.yml` | Weekly |
| cargo-deny advisory/policy audit | `maintenance.yml` | Weekly |
| Supported-platform compile checks (Windows, macOS) | `maintenance.yml` | Weekly |
| Latest-compatible dependency resolution | `latest-compatible.yml` | Weekly |
| Python eggcalc parity | `parity.yml` | Weekly |

### MSRV

Verifies the declared MSRV in `Cargo.toml` compiles and passes library tests. The canonical MSRV is defined once in `Cargo.toml` as `rust-version`.

### cargo-deny

Checks licenses, advisories, bans, and sources against `deny.toml`. Failures create maintainer tasks.

### Supported-platform compilation

Compile-checks on Windows and macOS to verify cross-platform compatibility. Uses `cargo check --locked --all-targets --all-features`.

### Latest-compatible

Runs `cargo update` to find the newest semver-compatible dependency set, then checks and tests. Detects upcoming ecosystem breakage.

### Python parity

Spawns both Rust and Python MCP servers, sends identical tool calls, and compares outputs. A failed workflow is the actionable output; the log provides version context.

## Discovery evaluation

The progressive-discovery rollout has a deterministic, offline gate in the
ordinary MCP integration test suite (`tests/mcp/test_discovery.rs`). It
measures exact serialized UTF-8 Tool definitions and runs the checked-in
intent corpus through the production lexical search function:

- full/Model direct advertises 77 tools; discovery advertises 7 (5 pinned
  front doors + `tool_search`/`tool_invoke`), currently ~5.44% of direct
  bytes with a <=25% gate;
- semantic coverage is derived from the registry, not hard-coded: all 76
  stable full/Model tools have task-oriented fixtures (100% coverage, 88
  positive intents including second phrasings for overlap-prone tools, 9
  HarnessOnly/Hidden containment fixtures);
- retrieval gates stay top-1 >=90%, top-3 >=98%, top-5 100%, zero
  Model-audience HarnessOnly/Hidden leaks, zero deprecated wins for
  migration targets, and no bare-canonical-name fixtures;
- the portable scenario corpus
  (`tests/fixtures/tool_discovery_scenarios.json`) keeps 48 Model task
  scenarios (40–60 required) plus 4 Model `must_not_expose` containment
  cases for the reclassified HarnessOnly targets; Harness tasks never join
  the Model denominator. Scenarios declare `audience`/`kind` and use the
  canonical plural `expected_tools`/`acceptable_tools` contract shared with
  traces.

Maintainer model/client evidence is recorded offline and never runs in merge
CI. The canonical trace contract uses plural `expected_tools` plus
`acceptable_tools`, per-scenario `success`, `selected_tool`,
`search_calls`, `tool_invoke_calls`, `mcp_calls`, `invalid_arguments`,
`wrong_tool_retries`, and header `model`, `provider`, `client`, `mode`,
`run_date`, `initial_tool_definition_bytes`, `server_instructions`
(`on|off`). Legacy singular `expected_tool` loads with a warning. Malformed
traces fail loudly (missing ids, empty tool sets, bad counters,
duplicate/mismatched scenario ids, cross-mode pair mismatches).

```bash
# Single-trace scoring (offline, provider-neutral):
python3 scripts/score-discovery-traces.py trace.json
# Matched direct-vs-discovery pair with noninferiority gates:
python3 scripts/score-discovery-traces.py --pair \
  tests/fixtures/discovery_traces/<date>-<model>-<client>-direct.json \
  tests/fixtures/discovery_traces/<date>-<model>-<client>-discovery.json
```

The pair must share model, provider, client, corpus revision, and
instructions treatment with opposite modes and identical scenario IDs.
Gates for recommending discovery: success within 2pp of direct, selection
within 2pp, no harmful invalid/retry regression, and observed search +
invoke use. See `tests/fixtures/discovery_traces/README.md` for the exact
maintainer procedure (same 48 Model tasks in both modes, sanitized fields
only, plus a 10–15-scenario instructions with/without A/B via a temporary
local build, <=500-byte instruction budget).

Model-driven results are evidence for rollout policy, not a networked merge
check. Generated client integrations stay on direct mode until paired OpenAI
and Anthropic traces plus the instructions A/B are recorded and show
noninferior success and host compatibility. As of this revision that
external evidence is still pending under
`plans/mcp-surface-03c-evaluation-closure-corrective.md`; the scorer and
corpus preparation above are complete, but the plan remains active.

## Tier 3 — Targeted Hardening

Run manually before material releases or after relevant implementation changes. Answers: "Are there latent defects in parser/regex/concurrency surfaces?"

| Check | When |
|-------|------|
| Extended fuzz matrices | Before releases touching parsing, regex, normalization |
| AddressSanitizer runs | Before releases touching memory-sensitive surfaces |
| Long concurrency/interleaving loops | After changes to execution lifecycle |

These checks are available but ordinary development must not wait on them unless they find a current reproducible defect.

## Tier 4 — Manual Release Verification

Run locally by the maintainer from the exact source intended for publication. Answers: "Is this version ready to publish?"

```bash
scripts/release-check.sh
```

This script runs the full verification gate locally and performs a `cargo publish --dry-run`. It never publishes, tags, or writes evidence files.

For a binary release, run the additional local checks before pushing the tag:

```bash
python3 scripts/check-release-contract.py
bash -n packaging/install.sh
shellcheck packaging/install.sh  # when available
cargo build --locked --release
./target/release/eggsact --version
python3 scripts/smoke-mcp-binary.py ./target/release/eggsact
```

The tag-only `release-binaries.yml` workflow repeats candidate `--version`,
`--help`, and MCP stdio smoke checks on every staged target, then creates only a
draft GitHub Release. It does not rerun the ordinary correctness matrix. The
workflow downloads Zig 0.14.1 with a pinned SHA-256 and installs
cargo-zigbuild 0.23.3 only in release jobs. Linux x86-64 targets the
documented glibc 2.17 floor; AArch64 uses the native `ubuntu-24.04-arm` runner
for both build and executable smoke. ARMv7 qualification remains a separate
claim. The v1.2.4 release is the first published binary-bearing release; its
exact-tag and `releases/latest/download` Unix installer paths were verified
after publication. ARMv7 remains Cargo fallback only.

## Failure Ownership

| Failure | Blocks merge? | Blocks release? | Expected response |
|---------|---------------|-----------------|-------------------|
| Linux correctness | yes | yes | fix before merge |
| Windows/macOS compile | no immediate PR block | yes for supported platforms | fix or change support policy |
| Weekly MSRV | no immediate PR block | yes if advertised MSRV is broken | repair or raise MSRV deliberately |
| cargo-deny advisory/policy | no immediate PR block | maintainer judgment; security findings normally block | triage dependency/policy |
| Python parity | no immediate PR block | blocks only when changed behavior promises parity | reproduce and classify drift |
| latest-compatible | no | no for locked release unless defect affects users | triage ecosystem drift |
| Fuzz/sanitizer crash | not automatically tied to unrelated PR | yes while reproducible and in release scope | minimize and fix/classify |

## Evidence Policy

Workflow logs are the evidence. Passing runs do not require documentation commits, run-ID transcription, artifact-digest recording, or package-count maintenance. A failed workflow is the actionable output.

## Ownership

- **Tier 1**: enforced by branch protection; all contributors
- **Tier 2**: maintainer responsibility; weekly triage of failures
- **Tier 3**: maintainer discretion before material releases
- **Tier 4**: maintainer action before every publication
