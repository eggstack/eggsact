pub mod cargo;
pub mod config;
pub mod confusables;
pub mod diff;
pub mod glob;
pub mod identifier;
pub mod inspect_prompt;
pub mod line_range;
pub mod markdown;
pub mod measure;
pub mod patch;
pub mod path;
pub mod position;
pub mod primitives;
pub mod regex_engine;
pub mod regex_safety;
pub mod replace;
pub mod shell;
pub mod synthesis;
pub mod toml;
pub mod transform;
pub mod unicode_policy;
pub mod unicode_tools;
pub mod validate;
pub mod version;

pub use regex_engine::{classify_pattern, RegexClassification, RegexEngineUsed, RegexFeature};
pub use regex_safety::{regex_safety_check, RegexSafetyResult};
pub use replace::{
    text_replace_check, text_replace_check_with_options, TextReplaceCheckOptions,
    TextReplaceCheckResult,
};

pub use cargo::{cargo_toml_inspect, CargoInspectResult};
pub use config::{
    dotenv_validate, ini_validate, DotenvEntry, DotenvValidateResult, IniKeyValueEntry,
    IniValidateResult,
};
pub use confusables::CONFUSABLES;
pub use confusables::{find_confusables, has_confusables};
pub use diff::{
    common_prefix_suffix, diff_spans, first_diff, levenshtein_distance, CommonPrefixSuffix,
    DiffSpan, FirstDiff,
};
pub use identifier::{
    identifier_analyze, identifier_inspect, identifier_table_inspect, IdentifierAnalyzeResult,
    IdentifierInspectResult, IdentifierTableInspectResult, TableIdentifierEntry,
};
pub use line_range::{
    line_range_compare, line_range_extract, FirstDifference, LineExtractFinding, LineExtractLine,
    LineRangeCompareResult, LineRangeExtractResult,
};
pub use markdown::{
    code_fence_extract, markdown_structure, CodeFenceBlock, CodeFenceExtractResult,
    MarkdownStructureResult,
};
pub use measure::{char_frequency, line_count, text_length, word_count};
pub use patch::{patch_apply_check, patch_summary, PatchApplyCheckResult, PatchSummaryResult};
pub use path::{
    path_analyze, path_compare, path_scope_check, PathAnalyzeResult, PathCompareResult,
    PathScopeCheckResult,
};
pub use primitives::{
    codepoint_index_to_byte_offset, codepoints, count_graphemes, truncate_to_grapheme,
    CodepointInfo,
};
pub use shell::{
    argv_compare, shell_quote_join, shell_split, ArgvCompareResult, ShellFeatures,
    ShellQuoteJoinResult, ShellSplitResult,
};
pub use toml::{toml_shape, validate_toml, TomlShapeResult, ValidateTomlResult};
pub use transform::{
    escape_text, text_fingerprint, text_hash, text_transform, unescape_text, EscapeTextResult,
    TextFingerprintResult, TextHashResult, TextTransformResult, UnescapeTextResult,
};
pub use unicode_policy::{
    canonicalize_text, unicode_policy_check, CanonicalizeResult, CanonicalizeResultWithMapping,
    PolicyFinding, UnicodePolicyCheckResult,
};
pub use validate::{
    json_canonicalize, json_compare, json_extract, json_shape, list_dedupe, list_sort,
    regex_finditer, regex_test, validate_brackets, validate_brackets_with_pairs, validate_json,
    validate_regex, validate_schema_light, CheckBracketsResult, JsonCanonicalizeResult,
    JsonCompareDiff, JsonCompareResult, JsonExtractResult, JsonShapeKey, JsonShapeResult,
    RegexFindIterMatch, RegexFindIterResult, RegexMatch, RegexTestResult, SchemaViolation,
    ValidateJsonResult, ValidateSchemaLightResult,
};
pub use version::{
    check_version_constraint, version_compare, VersionCompareResult, VersionConstraintResult,
};
