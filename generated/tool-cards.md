# Tool Cards

Generated from the ToolSpec registry. Each section corresponds to a codegg profile.

## `codegg_core_min`

### `command_preflight`

Composite: analyze a command before user approval or execution. Applies a policy engine (default/strict/permissive) with optional policy_config allow/deny overrides. Calls shell_split and regex_safety_check. Detects behavioral features (network, filesystem, process, env) and destructive patterns. Returns parsed argv, program, subcommand, features, risk findings, matched_rules, and a verdict. Must not execute anything.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core_min`
- **Composite**: yes
- **Required args**:
  - `command` (string)

### `config_preflight`

Composite: validate generated config text. Auto-detects format and runs the appropriate validator. Returns valid/invalid, detected format, parse error location, and machine code.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core_min`
- **Composite**: yes
- **Required args**:
  - `text` (string)

### `edit_preflight`

Composite: validate a proposed edit before applying it. Calls text_replace_check, patch_apply_check, line_range_extract, text_fingerprint, and text_diff_explain as needed. Optionally composes path_scope_check (when file_path + workspace_root are provided), text_fingerprint newline detection (when newline_policy is not "skip"), and text_security_inspect (when unicode_policy is not "skip"). Returns ok_to_apply verdict with findings and machine codes.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core_min`
- **Composite**: yes
- **Required args**:
  - `original` (string)

### `text_replace_check`

Check whether a text replacement would apply cleanly before an agent attempts to edit. Reports match count, positions, ambiguity, and optional preview of before/after.

- **Tier**: 1 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core_min`
- **Required args**:
  - `text` (string)
  - `old` (string)
  - `new` (string)

### `text_security_inspect`

Composite security-oriented text hygiene pass. Runs text_inspect, unicode_policy_check, canonicalize_text, prompt_input_inspect, and identifier_inspect depending on policy. Returns a verdict (allow/review/block) plus structured findings and machine codes.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core_min`
- **Composite**: yes
- **Required args**:
  - `text` (string)

### `validate_json`

Validate JSON and report precise parse errors or top-level structure information.

- **Tier**: 0 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core_min`
- **Required args**:
  - `text` (string)

## `codegg_core`

### `cargo_toml_inspect`

Inspect Cargo.toml text without network or filesystem access. Reports package metadata, workspace configuration, dependency forms (version/path/git/workspace), path dependencies, suspicious or confusable dependency names, and structural findings.

- **Tier**: 3 | **Cost**: mod | **Stability**: stable
- **Exposure**: expert
- **Profile**: `codegg_core`
- **Required args**:
  - `text` (string)

### `code_block_map`

Return approximate top-level block ranges (functions, classes, modules, headings) from source text or Markdown. Helps agents target edits without full parsing.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_core`
- **Required args**:
  - `source` (string)

### `command_preflight`

Composite: analyze a command before user approval or execution. Applies a policy engine (default/strict/permissive) with optional policy_config allow/deny overrides. Calls shell_split and regex_safety_check. Detects behavioral features (network, filesystem, process, env) and destructive patterns. Returns parsed argv, program, subcommand, features, risk findings, matched_rules, and a verdict. Must not execute anything.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Composite**: yes
- **Required args**:
  - `command` (string)

### `config_preflight`

Composite: validate generated config text. Auto-detects format and runs the appropriate validator. Returns valid/invalid, detected format, parse error location, and machine code.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Composite**: yes
- **Required args**:
  - `text` (string)

### `edit_preflight`

Composite: validate a proposed edit before applying it. Calls text_replace_check, patch_apply_check, line_range_extract, text_fingerprint, and text_diff_explain as needed. Optionally composes path_scope_check (when file_path + workspace_root are provided), text_fingerprint newline detection (when newline_policy is not "skip"), and text_security_inspect (when unicode_policy is not "skip"). Returns ok_to_apply verdict with findings and machine codes.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Composite**: yes
- **Required args**:
  - `original` (string)

### `identifier_inspect`

Inspect identifiers for validity and collisions. Detects confusables, mixed scripts, normalization issues, and casefold collisions across a list of identifiers.

- **Tier**: 1 | **Cost**: mod | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Required args**:
  - `identifiers` (array)

### `import_export_inspect`

Extract import/export/module-use statements from source text using lightweight language-aware heuristics. Supports Rust, Python, JavaScript/TypeScript, and Go.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_core`
- **Required args**:
  - `source` (string)

### `path_normalize`

Normalize a path using posixpath or ntpath semantics. Collapse dot segments, resolve components.

- **Tier**: 0 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Required args**:
  - `path` (string)

### `repo_language_detect`

Detect programming languages and ecosystems from repository file paths and manifest content. Uses file extensions, manifest presence, and optional content heuristics.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_core`
- **Required args**:
  - `paths` (array)

### `structured_data_compare`

Composite: compare structured config/data output. Calls json_compare, json_canonicalize, and json_shape. Returns equal/not-equal verdict with structured diffs.

- **Tier**: 2 | **Cost**: heavy | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_core`
- **Composite**: yes
- **Required args**:
  - `a` (string)
  - `b` (string)

### `test_command_suggest`

Suggest verification commands (build, test, lint, format) from repository paths and manifest content. Heuristic: returns confidence scores for each suggestion.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_core`
- **Required args**:
  - `paths` (array)

### `text_diff_explain`

Explain why two strings differ, including spans, codepoints, Unicode names, normalization equivalence, confusables, invisibles, and agent-facing classification.

- **Tier**: 1 | **Cost**: mod | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Required args**:
  - `a` (string)
  - `b` (string)

### `text_equal`

Compare two strings under raw, Unicode-normalized, casefolded, or trimmed modes and report exact equality evidence.

- **Tier**: 0 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Required args**:
  - `a` (string)
  - `b` (string)

### `text_fingerprint`

Compute a deterministic SHA-256 fingerprint of text with canonicalization options for Unicode normalization, newline style, casefold, and final newline trimming.

- **Tier**: 0 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Required args**:
  - `text` (string)

### `text_inspect`

Inspect a string for hidden characters, Unicode confusables, mixed scripts, normalization state, and display-safe representation. Can report both original and normalized text analysis.

- **Tier**: 1 | **Cost**: mod | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Required args**:
  - `text` (string)

### `text_replace_check`

Check whether a text replacement would apply cleanly before an agent attempts to edit. Reports match count, positions, ambiguity, and optional preview of before/after.

- **Tier**: 1 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Required args**:
  - `text` (string)
  - `old` (string)
  - `new` (string)

### `text_security_inspect`

Composite security-oriented text hygiene pass. Runs text_inspect, unicode_policy_check, canonicalize_text, prompt_input_inspect, and identifier_inspect depending on policy. Returns a verdict (allow/review/block) plus structured findings and machine codes.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Composite**: yes
- **Required args**:
  - `text` (string)

### `validate_json`

Validate JSON and report precise parse errors or top-level structure information.

- **Tier**: 0 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Required args**:
  - `text` (string)

### `validate_toml`

Validate TOML configuration files (Cargo.toml, pyproject.toml, etc.) and report parse errors with line/column positions.

- **Tier**: 1 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_core`
- **Required args**:
  - `text` (string)

## `codegg_preflight`

### `command_preflight`

Composite: analyze a command before user approval or execution. Applies a policy engine (default/strict/permissive) with optional policy_config allow/deny overrides. Calls shell_split and regex_safety_check. Detects behavioral features (network, filesystem, process, env) and destructive patterns. Returns parsed argv, program, subcommand, features, risk findings, matched_rules, and a verdict. Must not execute anything.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_preflight`
- **Composite**: yes
- **Required args**:
  - `command` (string)

### `config_preflight`

Composite: validate generated config text. Auto-detects format and runs the appropriate validator. Returns valid/invalid, detected format, parse error location, and machine code.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_preflight`
- **Composite**: yes
- **Required args**:
  - `text` (string)

### `dependency_edit_preflight`

Composite: inspect proposed dependency file changes before applying. Detects additions, removals, version changes, source changes (registry/path/git/url), script/hook changes, and patch overrides across Rust, Python, and Node ecosystems.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_preflight`
- **Composite**: yes
- **Required args**:
  - `file_path` (string)
  - `old_text` (string)
  - `new_text` (string)

### `edit_preflight`

Composite: validate a proposed edit before applying it. Calls text_replace_check, patch_apply_check, line_range_extract, text_fingerprint, and text_diff_explain as needed. Optionally composes path_scope_check (when file_path + workspace_root are provided), text_fingerprint newline detection (when newline_policy is not "skip"), and text_security_inspect (when unicode_policy is not "skip"). Returns ok_to_apply verdict with findings and machine codes.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_preflight`
- **Composite**: yes
- **Required args**:
  - `original` (string)

### `lockfile_inspect`

Inspect lockfile content or diffs for deterministic dependency-change signals. Detects added/removed/updated packages, git/path dependencies, and large churn. Not a vulnerability scanner.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_preflight`
- **Required args**: none

### `patch_contract_check`

Classify a unified diff by contract-relevant change categories (lockfiles, manifests, scope escapes, large deletions, security paths). Reports verdict and structured findings for automated routing.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_preflight`
- **Required args**:
  - `patch_text` (string)

### `text_security_inspect`

Composite security-oriented text hygiene pass. Runs text_inspect, unicode_policy_check, canonicalize_text, prompt_input_inspect, and identifier_inspect depending on policy. Returns a verdict (allow/review/block) plus structured findings and machine codes.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_preflight`
- **Composite**: yes
- **Required args**:
  - `text` (string)

## `codegg_patch`

### `diff_risk_classify`

Classify unified diffs by review risk and routing category. Reports risk categories, review focus items, and recommended next tools for reviewer agents.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_patch`
- **Required args**:
  - `patch_text` (string)

### `edit_preflight`

Composite: validate a proposed edit before applying it. Calls text_replace_check, patch_apply_check, line_range_extract, text_fingerprint, and text_diff_explain as needed. Optionally composes path_scope_check (when file_path + workspace_root are provided), text_fingerprint newline detection (when newline_policy is not "skip"), and text_security_inspect (when unicode_policy is not "skip"). Returns ok_to_apply verdict with findings and machine codes.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_patch`
- **Composite**: yes
- **Required args**:
  - `original` (string)

### `line_range_compare`

Compare a line range from two text inputs with exact, trailing-whitespace-ignoring, or newline-normalizing comparison.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_patch`
- **Required args**:
  - `left_text` (string)
  - `right_text` (string)
  - `start_line` (integer)
  - `end_line` (integer)

### `line_range_extract`

Extract exact line ranges from text and return stable offsets, byte positions, line counts, and optional fingerprint.

- **Tier**: 1 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_patch`
- **Required args**:
  - `text` (string)
  - `start_line` (integer)
  - `end_line` (integer)

### `lockfile_inspect`

Inspect lockfile content or diffs for deterministic dependency-change signals. Detects added/removed/updated packages, git/path dependencies, and large churn. Not a vulnerability scanner.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_patch`
- **Required args**: none

### `patch_contract_check`

Classify a unified diff by contract-relevant change categories (lockfiles, manifests, scope escapes, large deletions, security paths). Reports verdict and structured findings for automated routing.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_patch`
- **Required args**:
  - `patch_text` (string)

### `patch_summary`

Summarize a unified diff without applying it. Reports file counts, hunk counts, additions, deletions, renames, and line ranges by file.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_patch`
- **Required args**:
  - `patch_text` (string)

### `symbol_name_diff`

Compare old/new source text and report added, removed, and possibly renamed top-level symbols using brace/indentation-based heuristics.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_patch`
- **Required args**:
  - `old_source` (string)
  - `new_source` (string)

### `text_diff_explain`

Explain why two strings differ, including spans, codepoints, Unicode names, normalization equivalence, confusables, invisibles, and agent-facing classification.

- **Tier**: 1 | **Cost**: mod | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_patch`
- **Required args**:
  - `a` (string)
  - `b` (string)

### `text_replace_check`

Check whether a text replacement would apply cleanly before an agent attempts to edit. Reports match count, positions, ambiguity, and optional preview of before/after.

- **Tier**: 1 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_patch`
- **Required args**:
  - `text` (string)
  - `old` (string)
  - `new` (string)

## `codegg_config`

### `config_file_inspect`

Composite: inspect a single config file beyond syntax validity. Detects risky keys, secret-like values, insecure URLs, debug flags, command hooks, and TLS/hostname issues. Returns structured findings with severity and disposition.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_config`
- **Composite**: yes
- **Required args**:
  - `file_path` (string)
  - `text` (string)

### `config_preflight`

Composite: validate generated config text. Auto-detects format and runs the appropriate validator. Returns valid/invalid, detected format, parse error location, and machine code.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_config`
- **Composite**: yes
- **Required args**:
  - `text` (string)

### `dependency_edit_preflight`

Composite: inspect proposed dependency file changes before applying. Detects additions, removals, version changes, source changes (registry/path/git/url), script/hook changes, and patch overrides across Rust, Python, and Node ecosystems.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_config`
- **Composite**: yes
- **Required args**:
  - `file_path` (string)
  - `old_text` (string)
  - `new_text` (string)

### `dotenv_validate`

Validate .env-style key=value configuration text. Detects invalid keys, duplicate keys, missing quotes, and variable expansion syntax. Line-by-line parser, no shell evaluation.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_config`
- **Required args**:
  - `text` (string)

### `ini_validate`

Validate simple INI-style configuration files. Supports [section] headers, key=value and key:value lines, comments. Detects duplicate sections, duplicate keys, and malformed lines.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_config`
- **Required args**:
  - `text` (string)

### `json_canonicalize`

Canonicalize JSON with deterministic formatting, key ordering, duplicate key detection, and stable hashes.

- **Tier**: 1 | **Cost**: mod | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_config`
- **Required args**:
  - `text` (string)

### `json_compare`

Compare two JSON documents semantically, ignoring formatting and key order.

- **Tier**: 1 | **Cost**: mod | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_config`
- **Required args**:
  - `a` (string)
  - `b` (string)

### `json_extract`

Extract a value from JSON using RFC 6901 JSON Pointer (e.g., /foo/bar/0). Navigate nested objects and arrays.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_config`
- **Required args**:
  - `text` (string)

### `structured_data_compare`

Composite: compare structured config/data output. Calls json_compare, json_canonicalize, and json_shape. Returns equal/not-equal verdict with structured diffs.

- **Tier**: 2 | **Cost**: heavy | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_config`
- **Composite**: yes
- **Required args**:
  - `a` (string)
  - `b` (string)

### `toml_shape`

Analyze the structure of a TOML document: top-level keys, tables, and nesting hierarchy.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_config`
- **Required args**:
  - `text` (string)

### `validate_json`

Validate JSON and report precise parse errors or top-level structure information.

- **Tier**: 0 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_config`
- **Required args**:
  - `text` (string)

### `validate_schema_light`

Validate JSON against a simple schema format with type, required, enum, pattern, and nested constraints.

- **Tier**: 3 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_config`
- **Required args**:
  - `text` (string)
  - `schema` (object)

### `validate_toml`

Validate TOML configuration files (Cargo.toml, pyproject.toml, etc.) and report parse errors with line/column positions.

- **Tier**: 1 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_config`
- **Required args**:
  - `text` (string)

### `version_compare`

Compare two version strings with explicit scheme. Supports semver (major.minor.patch), loose (numeric parts), and deferred pep440.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_config`
- **Required args**:
  - `a` (string)
  - `b` (string)

## `codegg_unicode_security`

### `canonicalize_text`

Apply a named text canonicalization profile. Profiles include source_file_identity (NFC + LF + newline), identifier_compare (NFC + casefold), human_label_compare (NFC + casefold + whitespace collapse), json_key_compare (NFC + casefold), and path_segment_compare (NFC + lowercase + LF).

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_unicode_security`
- **Required args**:
  - `text` (string)
  - `profile` (string)

### `identifier_inspect`

Inspect identifiers for validity and collisions. Detects confusables, mixed scripts, normalization issues, and casefold collisions across a list of identifiers.

- **Tier**: 1 | **Cost**: mod | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_unicode_security`
- **Required args**:
  - `identifiers` (array)

### `text_inspect`

Inspect a string for hidden characters, Unicode confusables, mixed scripts, normalization state, and display-safe representation. Can report both original and normalized text analysis.

- **Tier**: 1 | **Cost**: mod | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_unicode_security`
- **Required args**:
  - `text` (string)

### `text_position`

Convert between byte offsets, codepoint indices, line/column positions, and UTF-16 offsets.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_unicode_security`
- **Required args**:
  - `text` (string)

### `text_security_inspect`

Composite security-oriented text hygiene pass. Runs text_inspect, unicode_policy_check, canonicalize_text, prompt_input_inspect, and identifier_inspect depending on policy. Returns a verdict (allow/review/block) plus structured findings and machine codes.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_unicode_security`
- **Composite**: yes
- **Required args**:
  - `text` (string)

### `text_transform`

Apply deterministic text transformations: Unicode normalization (NFC/NFD/NFKC/NFKD), casefold, trim, newline normalization, zero-width removal, bidi control stripping, and visible representation.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_unicode_security`
- **Required args**:
  - `text` (string)
  - `operations` (array)

## `codegg_shell`

### `argv_compare`

Compare two command strings or argv lists by parsed argv tokens rather than raw text. Supports command strings, pre-parsed argv lists, or both.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_shell`
- **Required args**: none

### `command_preflight`

Composite: analyze a command before user approval or execution. Applies a policy engine (default/strict/permissive) with optional policy_config allow/deny overrides. Calls shell_split and regex_safety_check. Detects behavioral features (network, filesystem, process, env) and destructive patterns. Returns parsed argv, program, subcommand, features, risk findings, matched_rules, and a verdict. Must not execute anything.

- **Tier**: 1 | **Cost**: heavy | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_shell`
- **Composite**: yes
- **Required args**:
  - `command` (string)

### `regex_safety_check`

Heuristic check for potential catastrophic backtracking risks in regex patterns. Flags nested quantifiers, repeated alternations, ambiguous dot-star, and backreferences.

- **Tier**: 1 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_shell`
- **Required args**:
  - `pattern` (string)

### `shell_quote_join`

Safely quote a list of argv tokens into a POSIX-like shell string. Verifies round-trip safety with shell_split.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_shell`
- **Required args**:
  - `argv` (array)

### `test_command_suggest`

Suggest verification commands (build, test, lint, format) from repository paths and manifest content. Heuristic: returns confidence scores for each suggestion.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_shell`
- **Required args**:
  - `paths` (array)

## `codegg_repo_audit`

### `cargo_toml_inspect`

Inspect Cargo.toml text without network or filesystem access. Reports package metadata, workspace configuration, dependency forms (version/path/git/workspace), path dependencies, suspicious or confusable dependency names, and structural findings.

- **Tier**: 3 | **Cost**: mod | **Stability**: stable
- **Exposure**: expert
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `text` (string)

### `code_block_map`

Return approximate top-level block ranges (functions, classes, modules, headings) from source text or Markdown. Helps agents target edits without full parsing.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `source` (string)

### `code_fence_extract`

Extract fenced code blocks from Markdown with exact line ranges, optional language filter, content, and SHA-256 fingerprints. Reports unclosed fences.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `text` (string)

### `config_file_inspect`

Composite: inspect a single config file beyond syntax validity. Detects risky keys, secret-like values, insecure URLs, debug flags, command hooks, and TLS/hostname issues. Returns structured findings with severity and disposition.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Composite**: yes
- **Required args**:
  - `file_path` (string)
  - `text` (string)

### `dependency_edit_preflight`

Composite: inspect proposed dependency file changes before applying. Detects additions, removals, version changes, source changes (registry/path/git/url), script/hook changes, and patch overrides across Rust, Python, and Node ecosystems.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Composite**: yes
- **Required args**:
  - `file_path` (string)
  - `old_text` (string)
  - `new_text` (string)

### `diff_risk_classify`

Classify unified diffs by review risk and routing category. Reports risk categories, review focus items, and recommended next tools for reviewer agents.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `patch_text` (string)

### `identifier_table_inspect`

Inspect a table of identifiers for casefold collisions, normalization collisions, confusable/near-collisions, style variants, reserved keyword hits, and mixed naming style groups. Accepts structured entries with name, kind, file, and line metadata.

- **Tier**: 3 | **Cost**: mod | **Stability**: stable
- **Exposure**: expert
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `identifiers` (array)

### `import_export_inspect`

Extract import/export/module-use statements from source text using lightweight language-aware heuristics. Supports Rust, Python, JavaScript/TypeScript, and Go.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `source` (string)

### `json_shape`

Analyze the structure of a JSON document without returning values. Shows type, keys, and nested structure with configurable depth limits.

- **Tier**: 3 | **Cost**: mod | **Stability**: stable
- **Exposure**: expert
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `text` (string)

### `lockfile_inspect`

Inspect lockfile content or diffs for deterministic dependency-change signals. Detects added/removed/updated packages, git/path dependencies, and large churn. Not a vulnerability scanner.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**: none

### `markdown_structure`

Parse Markdown structure with a deterministic line scanner: headings (level, text, slug), code fences (language, open/close state), links (visible vs target mismatch), HTML comments, frontmatter detection, and table detection. Not a full CommonMark parser.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `text` (string)

### `patch_contract_check`

Classify a unified diff by contract-relevant change categories (lockfiles, manifests, scope escapes, large deletions, security paths). Reports verdict and structured findings for automated routing.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `patch_text` (string)

### `repo_language_detect`

Detect programming languages and ecosystems from repository file paths and manifest content. Uses file extensions, manifest presence, and optional content heuristics.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `paths` (array)

### `repo_manifest_inspect`

Classify project manifests from a bounded path list. Detects Rust, Python, Node, Go, mixed, or unknown projects and emits tool hints for downstream inspection and command policy.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `paths` (array)

### `repo_tree_summarize`

Summarize repository shape from a bounded path list and optional metadata. Provides path bucketing, entrypoint/config/test/source/generated/vendor classification, and recommended next tools.

- **Tier**: 2 | **Cost**: mod | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `paths` (array)

### `symbol_name_diff`

Compare old/new source text and report added, removed, and possibly renamed top-level symbols using brace/indentation-based heuristics.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `old_source` (string)
  - `new_source` (string)

### `test_command_suggest`

Suggest verification commands (build, test, lint, format) from repository paths and manifest content. Heuristic: returns confidence scores for each suggestion.

- **Tier**: 2 | **Cost**: cheap | **Stability**: stable
- **Exposure**: contextual
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `paths` (array)

### `text_fingerprint`

Compute a deterministic SHA-256 fingerprint of text with canonicalization options for Unicode normalization, newline style, casefold, and final newline trimming.

- **Tier**: 0 | **Cost**: cheap | **Stability**: stable
- **Exposure**: default
- **Profile**: `codegg_repo_audit`
- **Required args**:
  - `text` (string)

