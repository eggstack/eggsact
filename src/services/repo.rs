//! Canonical repository facts shared by repo-analysis tools.
//!
//! One source of truth for ecosystem/path classification. The four
//! repo-analysis tools (`repo_manifest_inspect`, `repo_tree_summarize`,
//! `repo_language_detect`, `test_command_suggest`) and the patch-analysis
//! layer project different answers from [`RepoFacts`] instead of
//! independently re-detecting ecosystems, manifests, buckets, and languages.
//!
//! Layering: deterministic fact extraction lives here (no `ToolResponse`,
//! machine codes, verdicts, profiles, or MCP metadata). `tools/repo.rs`
//! adapters parse/validate their own input, call [`repo_facts`], and build
//! the existing wire shape once at the boundary. Suggested commands remain a
//! policy/projection layer in the adapter — they are not embedded in
//! [`RepoFacts`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Canonical ecosystem tables (single source of truth)
// ---------------------------------------------------------------------------

/// Rust manifest basenames + source hints.
///
/// `build.rs` is a manifest-adjacent build script; `src/main.rs`,
/// `src/lib.rs`, `src/bin/` are source hints matched case-insensitively via
/// substring (preserves the legacy `detect_project_types` semantics).
pub const RUST_MANIFESTS: &[&str] = &["Cargo.toml", "Cargo.lock", "build.rs"];
pub const RUST_SOURCE_HINTS: &[&str] = &["src/main.rs", "src/lib.rs", "src/bin/"];
pub const PYTHON_MANIFESTS: &[&str] = &[
    "pyproject.toml",
    "requirements.txt",
    "setup.cfg",
    "setup.py",
    "Pipfile",
    "poetry.lock",
];
pub const NODE_MANIFESTS: &[&str] = &[
    "package.json",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    ".npmrc",
];
pub const GO_MANIFESTS: &[&str] = &["go.mod", "go.sum"];

const CONFIG_PATTERNS: &[&str] = &[
    ".env",
    ".gitignore",
    ".editorconfig",
    "tsconfig.json",
    ".eslintrc",
    "rustfmt.toml",
    ".rustfmt.toml",
    "clippy.toml",
    ".clippy.toml",
    "Makefile",
    "Dockerfile",
    ".dockerignore",
];
const LOCKFILE_PATTERNS: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "Pipfile.lock",
    "go.sum",
];

/// Substrings that mark a path as security-sensitive (shared fact).
///
/// Centralized from `diff_risk_classify`. Whether a sensitive path blocks,
/// reviews, or is informational is policy decided by each consuming tool.
const SECURITY_SENSITIVE_SUBSTRINGS: &[&str] = &[
    "auth",
    "token",
    "secret",
    "crypto",
    "tls",
    "ssl",
    "permission",
    "policy",
    "sandbox",
    "exec",
    "shell",
];

// ---------------------------------------------------------------------------
// Canonical language table (single source of truth)
// ---------------------------------------------------------------------------

struct LanguageGuess {
    name: &'static str,
    extensions: &'static [&'static str],
}

const LANGUAGE_TABLE: &[LanguageGuess] = &[
    LanguageGuess {
        name: "rust",
        extensions: &[".rs"],
    },
    LanguageGuess {
        name: "python",
        extensions: &[".py", ".pyi", ".pyx"],
    },
    LanguageGuess {
        name: "javascript",
        extensions: &[".js", ".jsx", ".mjs", ".cjs"],
    },
    LanguageGuess {
        name: "typescript",
        extensions: &[".ts", ".tsx", ".mts", ".cts"],
    },
    LanguageGuess {
        name: "go",
        extensions: &[".go"],
    },
    LanguageGuess {
        name: "c",
        extensions: &[".c", ".h"],
    },
    LanguageGuess {
        name: "cpp",
        extensions: &[".cpp", ".cxx", ".cc", ".hpp", ".hxx"],
    },
    LanguageGuess {
        name: "java",
        extensions: &[".java"],
    },
    LanguageGuess {
        name: "ruby",
        extensions: &[".rb", ".erb"],
    },
    LanguageGuess {
        name: "php",
        extensions: &[".php"],
    },
    LanguageGuess {
        name: "swift",
        extensions: &[".swift"],
    },
    LanguageGuess {
        name: "kotlin",
        extensions: &[".kt", ".kts"],
    },
    LanguageGuess {
        name: "scala",
        extensions: &[".scala", ".sc"],
    },
    LanguageGuess {
        name: "haskell",
        extensions: &[".hs"],
    },
    LanguageGuess {
        name: "lua",
        extensions: &[".lua"],
    },
    LanguageGuess {
        name: "shell",
        extensions: &[".sh", ".bash", ".zsh"],
    },
    LanguageGuess {
        name: "sql",
        extensions: &[".sql"],
    },
    LanguageGuess {
        name: "markdown",
        extensions: &[".md", ".mdx"],
    },
    LanguageGuess {
        name: "yaml",
        extensions: &[".yaml", ".yml"],
    },
    LanguageGuess {
        name: "json",
        extensions: &[".json"],
    },
    LanguageGuess {
        name: "toml",
        extensions: &[".toml"],
    },
    LanguageGuess {
        name: "html",
        extensions: &[".html", ".htm"],
    },
    LanguageGuess {
        name: "css",
        extensions: &[".css", ".scss", ".less"],
    },
    LanguageGuess {
        name: "xml",
        extensions: &[".xml"],
    },
    LanguageGuess {
        name: "dockerfile",
        extensions: &[],
    },
    LanguageGuess {
        name: "makefile",
        extensions: &[],
    },
];

/// Per-language evidence derived deterministically from path extensions.
///
/// `confidence` preserves the legacy adapter scale (`0.9` for >5 files,
/// `0.7` for >1, else `0.5`) so wire output stays stable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LanguageEvidence {
    pub name: String,
    pub file_count: usize,
    pub extensions: Vec<String>,
    pub confidence: f64,
}

fn confidence_for_count(count: usize) -> f64 {
    if count > 5 {
        0.9
    } else if count > 1 {
        0.7
    } else {
        0.5
    }
}

/// Detect the language for a single path, if any.
pub fn language_for_path(path: &str) -> Option<String> {
    let basename = path.rsplit('/').next().unwrap_or(path);
    let lower = basename.to_lowercase();
    if lower == "dockerfile" || lower.starts_with("dockerfile.") {
        return Some("dockerfile".to_string());
    }
    if lower == "makefile" || lower == "gnumakefile" {
        return Some("makefile".to_string());
    }
    let ext = match basename.rfind('.') {
        Some(idx) => &basename[idx..],
        None => "",
    };
    LANGUAGE_TABLE
        .iter()
        .find(|l| l.extensions.contains(&ext))
        .map(|l| l.name.to_string())
}

/// Aggregate language evidence over a path list.
pub fn language_evidence(paths: &[String]) -> Vec<LanguageEvidence> {
    let mut counts: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for p in paths {
        let normalized = p.replace('\\', "/");
        let basename = normalized.rsplit('/').next().unwrap_or("").to_string();
        let Some(name) = language_for_path(&normalized) else {
            continue;
        };
        let entry = counts
            .entry(name.clone())
            .or_insert_with(|| (0, Vec::new()));
        entry.0 += 1;
        let ext_str = if name == "dockerfile" {
            "Dockerfile".to_string()
        } else if name == "makefile" {
            "Makefile".to_string()
        } else {
            basename
                .rfind('.')
                .map(|idx| basename[idx..].to_string())
                .unwrap_or_default()
        };
        if !ext_str.is_empty() && !entry.1.contains(&ext_str) {
            entry.1.push(ext_str);
        }
    }
    counts
        .into_iter()
        .map(|(name, (count, exts))| {
            // Preserve first-seen extension order (legacy adapter behavior).
            LanguageEvidence {
                name,
                confidence: confidence_for_count(count),
                file_count: count,
                extensions: exts,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Canonical ecosystem detection (single source of truth)
// ---------------------------------------------------------------------------

/// Detect project types from paths (canonical).
///
/// Preserves the legacy `repo_manifest_inspect`/`test_command_suggest`
/// semantics: Rust source hints (`src/main.rs`, `src/lib.rs`, `src/bin/`)
/// count alongside manifests; `mixed` is appended when more than one
/// ecosystem matches; `unknown` when none matches.
pub fn detect_project_types(paths: &[String]) -> Vec<String> {
    let mut has_rust = false;
    let mut has_python = false;
    let mut has_node = false;
    let mut has_go = false;

    for p in paths {
        let lower = p.to_lowercase();
        let basename = p.rsplit('/').next().unwrap_or(p);

        if RUST_MANIFESTS.contains(&basename) || RUST_SOURCE_HINTS.iter().any(|h| lower.contains(h))
        {
            has_rust = true;
        }
        if PYTHON_MANIFESTS.contains(&basename) {
            has_python = true;
        }
        if NODE_MANIFESTS.contains(&basename) {
            has_node = true;
        }
        if GO_MANIFESTS.contains(&basename) {
            has_go = true;
        }
    }

    let count = [has_rust, has_python, has_node, has_go]
        .iter()
        .filter(|&&b| b)
        .count();

    if count == 0 {
        vec!["unknown".to_string()]
    } else if count > 1 {
        let mut types = Vec::new();
        if has_rust {
            types.push("rust");
        }
        if has_python {
            types.push("python");
        }
        if has_node {
            types.push("node");
        }
        if has_go {
            types.push("go");
        }
        types.push("mixed");
        types.into_iter().map(String::from).collect()
    } else if has_rust {
        vec!["rust".to_string()]
    } else if has_python {
        vec!["python".to_string()]
    } else if has_node {
        vec!["node".to_string()]
    } else {
        vec!["go".to_string()]
    }
}

/// Ecosystems exposed by `repo_language_detect`: canonical project types
/// without the `unknown`/`mixed` markers.
pub fn detect_ecosystems(paths: &[String]) -> Vec<String> {
    detect_project_types(paths)
        .into_iter()
        .filter(|t| t != "unknown" && t != "mixed")
        .collect()
}

// ---------------------------------------------------------------------------
// Canonical path bucketing (single source of truth)
// ---------------------------------------------------------------------------

/// Classify a single repo-relative path into a bucket.
///
/// Bucket priority: manifests > lockfiles > ci > configs > tests >
/// generated > vendor > assets > scripts > docs > source.
/// Returns `(bucket, is_hidden, is_dotfile)`.
///
/// This is the canonical classifier. `tools::helpers::classify_path`
/// delegates here so repo-tree and diff-risk bucketing cannot drift.
pub fn classify_path(path: &str) -> (String, bool, bool) {
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

    let is_hidden = parts.iter().any(|p| p.starts_with('.'));
    let is_dotfile = parts.last().is_some_and(|p| p.starts_with('.'));

    let filename = *parts.last().unwrap_or(&"");
    let ext = filename.rsplit('.').next().unwrap_or("");
    let dir_path = normalized.to_lowercase();

    if matches!(
        filename,
        "Cargo.toml"
            | "package.json"
            | "pyproject.toml"
            | "setup.py"
            | "setup.cfg"
            | "go.mod"
            | "Gemfile"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "CMakeLists.txt"
            | "Makefile"
            | "makefile"
            | "Cargo.lock"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "package-lock.json"
            | "poetry.lock"
            | "Pipfile.lock"
            | "go.sum"
            | "Gemfile.lock"
    ) {
        if filename.ends_with("lock") || filename == "go.sum" || filename.contains("-lock.") {
            return ("lockfiles".to_string(), is_hidden, is_dotfile);
        }
        return ("manifests".to_string(), is_hidden, is_dotfile);
    }

    if dir_path.contains(".github/workflows")
        || dir_path.contains(".gitlab-ci")
        || dir_path.contains(".circleci")
        || dir_path.contains(".travis")
        || filename == "Dockerfile"
        || filename == "docker-compose.yml"
        || filename == "docker-compose.yaml"
        || filename == ".dockerignore"
        || filename == "Jenkinsfile"
        || filename == ".travis.yml"
        || filename == "appveyor.yml"
    {
        return ("ci".to_string(), is_hidden, is_dotfile);
    }

    if matches!(
        filename,
        ".editorconfig"
            | ".gitignore"
            | ".gitattributes"
            | ".cargo/config.toml"
            | "rustfmt.toml"
            | "clippy.toml"
            | ".rustfmt.toml"
            | ".clippy.toml"
            | "tsconfig.json"
            | ".eslintrc"
            | ".eslintrc.json"
            | ".eslintrc.js"
            | ".prettierrc"
            | ".prettierrc.json"
            | "babel.config.js"
            | "webpack.config.js"
            | ".env"
            | ".env.example"
            | "tox.ini"
            | "mypy.ini"
            | ".flake8"
            | "pytest.ini"
            | "conftest.py"
    ) || (ext == "toml"
        && !filename.starts_with('.')
        && (filename.contains("config") || filename.contains("Cargo")))
    {
        return ("configs".to_string(), is_hidden, is_dotfile);
    }

    let is_test_file = filename.starts_with("test_")
        || filename.starts_with("test.")
        || filename.ends_with("_test.rs")
        || filename.ends_with("_test.py")
        || filename.ends_with(".test.js")
        || filename.ends_with(".test.ts")
        || filename.ends_with(".spec.js")
        || filename.ends_with(".spec.ts")
        || (filename.contains("test") && ext == "rs")
        || filename == "tests.rs";

    let is_test_dir = dir_path.contains("/test/")
        || dir_path.contains("/tests/")
        || dir_path.contains("/__tests__/")
        || dir_path.contains("/test_data/")
        || dir_path.contains("/testdata/")
        || dir_path.contains("/fixtures/")
        || dir_path.contains("/snapshots/");

    if is_test_file || is_test_dir {
        return ("tests".to_string(), is_hidden, is_dotfile);
    }

    let is_generated = dir_path.contains("/generated/")
        || dir_path.contains("/gen/")
        || dir_path.contains("/dist/")
        || dir_path.contains("/build/")
        || dir_path.contains("/target/")
        || dir_path.contains("/out/")
        || dir_path.contains("/.next/")
        || dir_path.contains("/__pycache__/")
        || filename.ends_with(".min.js")
        || filename.ends_with(".min.css")
        || filename.ends_with(".bundle.js")
        || ext == "lock"
        || filename == "package-lock.json"
        || filename == "yarn.lock";

    if is_generated {
        return ("generated".to_string(), is_hidden, is_dotfile);
    }

    let is_vendor = dir_path.contains("/vendor/")
        || dir_path.contains("/node_modules/")
        || dir_path.contains("/.vendor/")
        || dir_path.contains("/third_party/")
        || dir_path.contains("/third-party/")
        || dir_path.contains("/extern/")
        || dir_path.contains("/external/")
        || dir_path.contains("/deps/");

    if is_vendor {
        return ("vendor".to_string(), is_hidden, is_dotfile);
    }

    if matches!(
        ext,
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "svg"
            | "ico"
            | "webp"
            | "bmp"
            | "tiff"
            | "woff"
            | "woff2"
            | "ttf"
            | "eot"
            | "otf"
            | "mp3"
            | "mp4"
            | "wav"
            | "avi"
            | "mov"
            | "pdf"
            | "zip"
            | "tar"
            | "gz"
    ) {
        return ("assets".to_string(), is_hidden, is_dotfile);
    }

    if matches!(
        filename,
        "release.sh"
            | "deploy.sh"
            | "build.sh"
            | "test.sh"
            | "ci.sh"
            | "setup.sh"
            | "install.sh"
            | "bootstrap.sh"
    ) || (ext == "sh" && (dir_path.contains("script") || dir_path.contains("bin")))
    {
        return ("scripts".to_string(), is_hidden, is_dotfile);
    }

    if matches!(ext, "md" | "mdx" | "rst" | "txt" | "adoc" | "asciidoc")
        || matches!(
            filename,
            "README"
                | "LICENSE"
                | "LICENSE-MIT"
                | "LICENSE-APACHE"
                | "CONTRIBUTING"
                | "CHANGELOG"
                | "CHANGES"
                | "AUTHORS"
                | "SECURITY"
                | "CODE_OF_CONDUCT"
                | ".mailmap"
        )
        || dir_path.contains("/docs/")
        || dir_path.contains("/doc/")
        || dir_path.contains("/documentation/")
    {
        return ("docs".to_string(), is_hidden, is_dotfile);
    }

    ("source".to_string(), is_hidden, is_dotfile)
}

// --- Fact predicates (raw classification, not policy) ----------------------

/// Canonical bucket for a path (convenience over [`classify_path`]).
pub fn path_bucket(path: &str) -> String {
    classify_path(path).0
}

pub fn is_manifest_bucket(path: &str) -> bool {
    path_bucket(path) == "manifests"
}
pub fn is_lockfile_bucket(path: &str) -> bool {
    path_bucket(path) == "lockfiles"
}
pub fn is_ci_path(path: &str) -> bool {
    path_bucket(path) == "ci"
}
pub fn is_config_bucket(path: &str) -> bool {
    path_bucket(path) == "configs"
}
pub fn is_generated_path(path: &str) -> bool {
    path_bucket(path) == "generated"
}
pub fn is_vendor_path(path: &str) -> bool {
    path_bucket(path) == "vendor"
}

/// Shared security-sensitive fact: path contains a sensitive substring.
///
/// Policy (block vs review vs informational) is decided by each consumer;
/// this predicate only reports the raw classification.
pub fn is_security_sensitive_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    SECURITY_SENSITIVE_SUBSTRINGS
        .iter()
        .any(|s| lower.contains(s))
}

// ---------------------------------------------------------------------------
// Manifest grouping (canonical)
// ---------------------------------------------------------------------------

/// Group manifest/lockfile/config paths by ecosystem (canonical).
///
/// Preserves the legacy `classify_manifests` 7-tuple semantics so
/// `repo_manifest_inspect` output stays stable.
#[allow(clippy::type_complexity)]
pub fn classify_manifests(
    paths: &[String],
) -> (
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
) {
    let mut rust_manifests = Vec::new();
    let mut python_manifests = Vec::new();
    let mut node_manifests = Vec::new();
    let mut go_manifests = Vec::new();
    let mut other_manifests = Vec::new();
    let mut config_paths = Vec::new();
    let mut lockfile_paths = Vec::new();

    for p in paths {
        let basename = p.rsplit('/').next().unwrap_or(p);

        if RUST_MANIFESTS.contains(&basename) {
            rust_manifests.push(p.clone());
        } else if PYTHON_MANIFESTS.contains(&basename) {
            python_manifests.push(p.clone());
        } else if NODE_MANIFESTS.contains(&basename) {
            node_manifests.push(p.clone());
        } else if GO_MANIFESTS.contains(&basename) {
            go_manifests.push(p.clone());
        } else if LOCKFILE_PATTERNS.contains(&basename) {
            lockfile_paths.push(p.clone());
        } else if CONFIG_PATTERNS.iter().any(|m| basename.contains(m)) {
            config_paths.push(p.clone());
        } else if basename.contains("config")
            || basename.contains("rc")
            || basename.starts_with('.')
        {
            other_manifests.push(p.clone());
        }
    }

    (
        rust_manifests,
        python_manifests,
        node_manifests,
        go_manifests,
        other_manifests,
        config_paths,
        lockfile_paths,
    )
}

// ---------------------------------------------------------------------------
// RepoFacts
// ---------------------------------------------------------------------------

/// Reusable repository observations (facts, not verdicts).
///
/// No `ToolResponse`, machine codes, verdicts, profiles, or MCP metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepoFacts {
    /// Input paths normalized (`\` -> `/`).
    pub normalized_paths: Vec<String>,
    /// Bucket name -> paths (only non-empty buckets are present).
    pub buckets: BTreeMap<String, Vec<String>>,
    /// Canonical project types (`rust`/`python`/`node`/`go` + `mixed`/`unknown`).
    pub project_types: Vec<String>,
    /// Ecosystems without the `unknown`/`mixed` markers.
    pub ecosystems: Vec<String>,
    /// Manifest paths grouped by ecosystem.
    pub manifests_by_ecosystem: BTreeMap<String, Vec<String>>,
    /// Non-ecosystem manifest-like paths.
    pub other_manifests: Vec<String>,
    /// Config paths (legacy manifest-inspect grouping).
    pub config_paths: Vec<String>,
    /// Lockfile paths (legacy manifest-inspect grouping).
    pub lockfile_paths: Vec<String>,
    /// Likely entry files.
    pub entrypoint_candidates: Vec<String>,
    /// CI/security/build paths needing attention.
    pub high_leverage_paths: Vec<String>,
    /// Recommended follow-up tools (derived from buckets/manifests).
    pub tool_hints: Vec<String>,
    /// Language evidence by extension.
    pub languages: Vec<LanguageEvidence>,
    /// True when no ecosystem matched.
    pub is_unknown: bool,
    /// True when more than one ecosystem matched.
    pub is_mixed: bool,
}

/// Compute [`RepoFacts`] for explicit input paths.
///
/// Pure, deterministic, no filesystem access. Callers enforce
/// `max_paths`/length bounds before calling.
pub fn repo_facts(paths: &[String]) -> RepoFacts {
    let normalized_paths: Vec<String> = paths.iter().map(|p| p.replace('\\', "/")).collect();

    let project_types = detect_project_types(paths);
    let ecosystems = detect_ecosystems(paths);
    let is_unknown = project_types.iter().any(|t| t == "unknown");
    let is_mixed = project_types.iter().any(|t| t == "mixed");

    let (
        rust_manifests,
        python_manifests,
        node_manifests,
        go_manifests,
        other_manifests,
        config_paths,
        lockfile_paths,
    ) = classify_manifests(paths);

    let mut manifests_by_ecosystem = BTreeMap::new();
    manifests_by_ecosystem.insert("rust".to_string(), rust_manifests);
    manifests_by_ecosystem.insert("python".to_string(), python_manifests);
    manifests_by_ecosystem.insert("node".to_string(), node_manifests);
    manifests_by_ecosystem.insert("go".to_string(), go_manifests);

    let mut buckets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut entrypoint_candidates = Vec::new();
    let mut high_leverage_paths = Vec::new();
    let mut tool_hints = Vec::new();

    for (original, normalized) in paths.iter().zip(normalized_paths.iter()) {
        let (bucket, _hidden, _dotfile) = classify_path(original);
        buckets
            .entry(bucket.clone())
            .or_default()
            .push(original.clone());

        let filename = normalized.rsplit('/').next().unwrap_or("");
        if matches!(
            filename,
            "src/main.rs"
                | "src/lib.rs"
                | "main.py"
                | "app.py"
                | "index.js"
                | "index.ts"
                | "src/index.ts"
                | "src/index.js"
                | "cmd/main.go"
                | "main.go"
        ) || filename == "package.json"
            || filename == "pyproject.toml"
            || filename == "Cargo.toml"
            || filename == "go.mod"
        {
            entrypoint_candidates.push(original.clone());
        }

        if bucket == "ci"
            || bucket == "manifests"
            || bucket == "lockfiles"
            || filename.contains("security")
            || filename.contains("auth")
            || filename.contains("Dockerfile")
            || filename.contains("release")
        {
            high_leverage_paths.push(original.clone());
        }

        if filename == "Cargo.toml" {
            tool_hints.push("cargo_toml_inspect".to_string());
        }
        if filename.ends_with(".toml") || filename.ends_with(".json") || filename.ends_with(".yaml")
        {
            tool_hints.push("config_file_inspect".to_string());
        }
        if bucket == "manifests" || bucket == "lockfiles" {
            tool_hints.push("dependency_edit_preflight".to_string());
        }
        if bucket == "docs" && filename.ends_with(".md") {
            tool_hints.push("markdown_structure".to_string());
            tool_hints.push("code_fence_extract".to_string());
        }
    }

    tool_hints.sort();
    tool_hints.dedup();

    // Preserve legacy `classify_paths` ordering: bucket values,
    // entrypoints, and high-leverage paths stay in input order (no sorting
    // or dedup) so existing wire output is byte-stable. Only `tool_hints`
    // is sorted/deduped, matching the legacy behavior.

    let languages = language_evidence(paths);

    RepoFacts {
        normalized_paths,
        buckets,
        project_types,
        ecosystems,
        manifests_by_ecosystem,
        other_manifests,
        config_paths,
        lockfile_paths,
        entrypoint_candidates,
        high_leverage_paths,
        tool_hints,
        languages,
        is_unknown,
        is_mixed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn canonical_project_types_rust() {
        let facts = repo_facts(&s(&["Cargo.toml", "src/main.rs"]));
        assert!(facts.project_types.contains(&"rust".to_string()));
        assert!(!facts.is_unknown);
    }

    #[test]
    fn canonical_project_types_mixed() {
        let facts = repo_facts(&s(&["Cargo.toml", "package.json"]));
        assert!(facts.is_mixed);
        assert!(facts.project_types.contains(&"mixed".to_string()));
    }

    #[test]
    fn canonical_project_types_unknown() {
        let facts = repo_facts(&s(&["README.md"]));
        assert!(facts.is_unknown);
        assert_eq!(facts.project_types, vec!["unknown".to_string()]);
    }

    #[test]
    fn source_hint_implies_rust_everywhere() {
        // Single source of truth: src/main.rs alone is rust for both the
        // manifest-style project types and the language-style ecosystems.
        let facts = repo_facts(&s(&["src/main.rs"]));
        assert!(facts.project_types.contains(&"rust".to_string()));
        assert!(facts.ecosystems.contains(&"rust".to_string()));
    }

    #[test]
    fn buckets_use_canonical_classifier() {
        let facts = repo_facts(&s(&[".github/workflows/ci.yml", "src/main.rs"]));
        assert!(facts.buckets.contains_key("ci"));
        assert!(facts
            .high_leverage_paths
            .contains(&".github/workflows/ci.yml".to_string()));
    }

    #[test]
    fn manifest_grouping_stable() {
        let facts = repo_facts(&s(&["Cargo.toml", "package.json", "go.mod"]));
        assert_eq!(
            facts.manifests_by_ecosystem["rust"],
            vec!["Cargo.toml".to_string()]
        );
        assert_eq!(
            facts.manifests_by_ecosystem["node"],
            vec!["package.json".to_string()]
        );
        assert_eq!(
            facts.manifests_by_ecosystem["go"],
            vec!["go.mod".to_string()]
        );
    }

    #[test]
    fn security_fact_centralized() {
        assert!(is_security_sensitive_path("src/auth/handler.rs"));
        assert!(!is_security_sensitive_path("src/main.rs"));
    }

    #[test]
    fn language_evidence_counts() {
        let langs = language_evidence(&s(&["a.rs", "b.rs", "c.py"]));
        let rust = langs.iter().find(|l| l.name == "rust").unwrap();
        assert_eq!(rust.file_count, 2);
    }
}
