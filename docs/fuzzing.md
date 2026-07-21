# Fuzzing and Property Testing

Release 5 adds persistent fuzz corpora, targeted property tests, and reproducible triage workflows.

## Prerequisites

Install cargo-fuzz and the nightly toolchain:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz --locked
```

Fuzz targets require the nightly toolchain because libFuzzer uses unstable Rust features.

## Fuzz Targets

| Target | Module | What it fuzzes |
|--------|--------|----------------|
| `calculator_expression` | `src/calc/` | Expression parser/evaluator |
| `calculator_normalization` | `src/calc/` | Normalization/tokenization |
| `unified_diff` | `src/text/patch.rs` | Unified diff parser |
| `shell_tokenization` | `src/text/shell.rs` | Shell command splitting |
| `shell_quoting` | `src/text/shell.rs` | Quote/parse round-trips |
| `regex_classification` | `src/text/regex_engine.rs` | Regex feature classifier |
| `regex_execution` | `src/text/validate.rs` | Regex compile and match |
| `json_pointer` | `src/text/validate.rs` | JSON parse/extract/canonicalize |
| `toml_config` | `src/text/toml.rs`, `config.rs` | TOML/dotenv/INI validation |
| `unicode_inspection` | `src/text/unicode_*.rs` | Unicode normalization/inspection |
| `markdown_fences` | `src/text/markdown.rs` | Markdown structure extraction |
| `glob_matching` | `src/text/glob.rs`, `path.rs` | Glob and path operations |

## Running Fuzz Targets

### Build all targets

```bash
cargo fuzz build
```

### Run a specific target

```bash
cargo fuzz run calculator_expression -- -max_total_time=60 -timeout=5
```

### Run with specific corpus

```bash
cargo fuzz run calculator_expression fuzz/corpus/calculator_expression/
```

### Run with limited iterations

```bash
cargo fuzz run unified_diff -- -max_total_time=30 -runs=10000
```

## Corpus Policy

### What to commit

- Seed corpus files in `fuzz/corpus/<target>/` (small, representative inputs)
- Minimized regression fixtures from fixed bugs
- Historical test cases and edge cases

### What NOT to commit

- Raw crash artifacts (large or unreviewed)
- Generated corpora from long fuzz campaigns
- Multi-megabyte files without justification

### Seed sources

- Existing unit/integration test fixtures
- Historical bug reproductions
- Python parity edge cases
- Machine-code error cases
- Boundary inputs (min/max lengths, empty, etc.)

## Crash Workflow

For every crash, timeout, OOM, or invariant failure:

1. **Preserve** the raw artifact locally
2. **Reproduce** with the exact target
3. **Minimize** using `cargo fuzz tmin`:
   ```bash
   cargo fuzz tmin calculator_expression <crash-file>
   ```
4. **Classify** the finding:
   - Production bug (fix in src/)
   - Harness bug (fix in fuzz_targets/)
   - Expected resource rejection (document and filter)
   - Duplicate of known issue
5. **Fix** the production or harness bug
6. **Add regression test** in `tests/`
7. **Add minimized input** to persistent corpus
8. **Re-run** target and full test gate

## Property Tests

Property tests run in ordinary CI via `cargo test`. They verify algebraic properties:

- **Calculator**: determinism, context isolation, normalization idempotence
- **Diff**: Levenshtein symmetry, triangle inequality, span bounds
- **Shell**: round-trip `parse(quote(argv)) == argv`, determinism
- **Regex**: classification determinism, span bounds, max_matches
- **JSON**: canonicalization idempotence, compare symmetry, extract bounds
- **Config**: validation determinism
- **Unicode**: NFC idempotence, grapheme bounds, casefold validity
- **Markdown**: fence span ordering, extraction determinism
- **Path/Glob**: normalization idempotence, matching determinism

Run property tests:
```bash
cargo test --locked --all-features property
```

## CI Integration

### PR Smoke Fuzzing

A lightweight job runs selected targets for 30 seconds each on every PR:
- `calculator_expression`
- `unified_diff`
- `shell_tokenization`
- `regex_classification`
- `json_pointer`
- `unicode_inspection`

### Scheduled Extended Fuzzing

A weekly workflow runs all targets for 5 minutes each with full sanitizer coverage.

## Sanitizers

AddressSanitizer and LeakSanitizer are available via cargo-fuzz:

```bash
cargo fuzz run calculator_expression --sanitizer=address
cargo fuzz run calculator_expression --sanitizer=leak
```

## Security Handling

If a fuzz finding has plausible security impact:

1. Do not publish exploit details before assessment
2. Use the repository's security reporting process
3. Minimize and fix privately if required
4. Add a public regression test only after disclosure policy permits
