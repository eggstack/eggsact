use regex::Regex;
use serde::{Deserialize, Serialize};

static DOMAIN_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[\w.-]+\.[\w]{2,}$").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownHeading {
    pub level: usize,
    pub text: String,
    pub line: usize,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownCodeFence {
    pub language: String,
    #[serde(rename = "start_line")]
    pub start_line: usize,
    #[serde(rename = "end_line")]
    pub end_line: Option<usize>,
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownLink {
    #[serde(rename = "visible_text")]
    pub visible_text: String,
    pub target: String,
    pub line: usize,
    #[serde(rename = "mismatch_flags")]
    pub mismatch_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownFrontmatter {
    pub present: bool,
    pub format: String,
    #[serde(rename = "line_start")]
    pub line_start: Option<usize>,
    #[serde(rename = "line_end")]
    pub line_end: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownHtmlComment {
    pub text: String,
    pub line: usize,
    #[serde(rename = "start_col")]
    pub start_col: usize,
    #[serde(rename = "end_col")]
    pub end_col: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownStructureResult {
    pub headings: Vec<MarkdownHeading>,
    #[serde(rename = "code_fences")]
    pub code_fences: Vec<MarkdownCodeFence>,
    pub links: Vec<MarkdownLink>,
    #[serde(rename = "html_comments")]
    pub html_comments: Vec<MarkdownHtmlComment>,
    pub frontmatter: MarkdownFrontmatter,
    #[serde(rename = "tables_detected")]
    pub tables_detected: bool,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFenceBlock {
    pub index: usize,
    pub language: String,
    #[serde(rename = "start_line")]
    pub start_line: usize,
    #[serde(rename = "end_line")]
    pub end_line: Option<usize>,
    pub closed: bool,
    pub content: Option<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFenceExtractResult {
    pub blocks: Vec<CodeFenceBlock>,
    #[serde(rename = "unclosed_fences")]
    pub unclosed_fences: Vec<serde_json::Value>,
    pub findings: Vec<String>,
}

use std::sync::LazyLock;

static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+?)(?:\s+#+)?\s*$").unwrap());
static CODE_FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(`{3,}|~{3,})(.*)$").unwrap());
static LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap());
static HTML_COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<!--.*?-->").unwrap());
static TABLE_SEPARATOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\|?\s*[-:]+[-| :]*$").unwrap());
static SLUG_NON_WORD_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"[^\w\s-]").unwrap());
static SLUG_WHITESPACE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"[\s]+").unwrap());
static SLUG_DASH_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"-+").unwrap());

fn make_slug(text: &str) -> String {
    let mut slug = text.to_lowercase();
    slug = slug.trim().to_string();
    slug = SLUG_NON_WORD_RE.replace_all(&slug, "").to_string();
    slug = SLUG_WHITESPACE_RE.replace_all(&slug, "-").to_string();
    slug = SLUG_DASH_RE.replace_all(&slug, "-").to_string();
    slug.trim_matches('-').to_string()
}

pub fn markdown_structure(
    text: &str,
    include_sections: bool,
    include_links: bool,
    include_code_fences: bool,
    include_html_comments: bool,
) -> MarkdownStructureResult {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut headings: Vec<MarkdownHeading> = Vec::new();
    let mut code_fences: Vec<MarkdownCodeFence> = Vec::new();
    let mut links: Vec<MarkdownLink> = Vec::new();
    let mut html_comments: Vec<MarkdownHtmlComment> = Vec::new();
    let mut findings: Vec<String> = Vec::new();

    let mut tables_detected = false;

    let mut in_fence = false;
    let mut fence_char = ' ';
    let mut fence_len = 0;
    let mut fence_start_line = 0;
    let mut fence_lang = String::new();

    let mut in_frontmatter = false;
    let mut frontmatter_format = "unknown".to_string();
    let mut frontmatter_line_start: Option<usize> = None;
    let mut frontmatter_line_end: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        let stripped = line.trim();

        if i == 0 {
            if stripped == "---" {
                in_frontmatter = true;
                frontmatter_format = "yaml".to_string();
                frontmatter_line_start = Some(line_num);
            } else if stripped == "+++" {
                in_frontmatter = true;
                frontmatter_format = "toml".to_string();
                frontmatter_line_start = Some(line_num);
            }
            if in_frontmatter {
                continue;
            }
        }

        if in_frontmatter {
            let stripped = stripped.to_string();
            if (stripped == "---" && frontmatter_format == "yaml")
                || (stripped == "+++" && frontmatter_format == "toml")
            {
                frontmatter_line_end = Some(line_num);
                in_frontmatter = false;
                continue;
            }
            continue;
        }

        if include_code_fences {
            if let Some(fence_match) = CODE_FENCE_RE.captures(stripped) {
                let fence_opener = &fence_match[1];
                let lang = fence_match[2].trim();
                let current_fence_char = fence_opener.chars().next().unwrap();
                let current_fence_len = fence_opener.len();

                if !in_fence {
                    in_fence = true;
                    fence_char = current_fence_char;
                    fence_len = current_fence_len;
                    fence_start_line = line_num;
                    fence_lang = lang.to_string();
                } else if current_fence_char == fence_char && current_fence_len >= fence_len {
                    code_fences.push(MarkdownCodeFence {
                        language: fence_lang.clone(),
                        start_line: fence_start_line,
                        end_line: Some(line_num),
                        closed: true,
                    });
                    in_fence = false;
                }
                continue;
            }
        }

        if include_sections && !in_fence {
            if let Some(heading_match) = HEADING_RE.captures(stripped) {
                let level = heading_match[1].len();
                let text_content = heading_match[2].trim();
                headings.push(MarkdownHeading {
                    level,
                    text: text_content.to_string(),
                    line: line_num,
                    slug: make_slug(text_content),
                });
            }
        }

        if include_links && !in_fence {
            for link_match in LINK_RE.captures_iter(stripped) {
                let visible = &link_match[1];
                let target = &link_match[2];
                let mut mismatch_flags: Vec<String> = Vec::new();

                if (visible.starts_with("http://") || visible.starts_with("https://"))
                    && visible != target
                {
                    mismatch_flags.push("visible_is_url".to_string());
                }

                if DOMAIN_RE.is_match(visible) && visible != target {
                    mismatch_flags.push("visible_is_domain".to_string());
                }

                links.push(MarkdownLink {
                    visible_text: visible.to_string(),
                    target: target.to_string(),
                    line: line_num,
                    mismatch_flags,
                });
            }
        }

        if include_html_comments && !in_fence {
            for comment_match in HTML_COMMENT_RE.captures_iter(stripped) {
                let comment_text = &comment_match[0];
                let match_start = comment_match.get(0).map(|m| m.start()).unwrap_or(0);
                let match_end = comment_match.get(0).map(|m| m.end()).unwrap_or(0);
                let start_col = stripped[..match_start].chars().count() + 1;
                let end_col = stripped[..match_end].chars().count() + 1;
                html_comments.push(MarkdownHtmlComment {
                    text: comment_text.to_string(),
                    line: line_num,
                    start_col,
                    end_col,
                });
            }
        }

        if !tables_detected && !in_fence && TABLE_SEPARATOR_RE.is_match(stripped) {
            for j in (0..i).rev() {
                let prev = lines[j].trim();
                if !prev.is_empty() {
                    if prev.contains('|') {
                        tables_detected = true;
                    }
                    break;
                }
            }
        }
    }

    if in_fence {
        code_fences.push(MarkdownCodeFence {
            language: fence_lang,
            start_line: fence_start_line,
            end_line: None,
            closed: false,
        });
        findings.push(format!(
            "Unclosed code fence starting at line {}",
            fence_start_line
        ));
    }

    if in_frontmatter {
        findings.push("Unclosed frontmatter block".to_string());
    }

    MarkdownStructureResult {
        headings,
        code_fences,
        links,
        html_comments,
        frontmatter: MarkdownFrontmatter {
            present: in_frontmatter || frontmatter_line_start.is_some(),
            format: frontmatter_format,
            line_start: frontmatter_line_start,
            line_end: frontmatter_line_end,
        },
        tables_detected,
        findings,
    }
}

fn fingerprint(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn code_fence_extract(
    text: &str,
    language: Option<&str>,
    include_content: bool,
) -> CodeFenceExtractResult {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut blocks: Vec<CodeFenceBlock> = Vec::new();
    let mut unclosed_fences: Vec<serde_json::Value> = Vec::new();
    let mut findings: Vec<String> = Vec::new();
    let mut index = 0;

    let mut in_fence = false;
    let mut fence_char = ' ';
    let mut fence_len = 0;
    let mut fence_start_line = 0;
    let mut fence_lang = String::new();
    let mut fence_content_lines: Vec<String> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        let stripped = line.trim();

        if let Some(fence_match) = CODE_FENCE_RE.captures(stripped) {
            let fence_opener = &fence_match[1];
            let lang = fence_match[2].trim();
            let current_fence_char = fence_opener.chars().next().unwrap();
            let current_fence_len = fence_opener.len();

            if !in_fence {
                in_fence = true;
                fence_char = current_fence_char;
                fence_len = current_fence_len;
                fence_start_line = line_num;
                fence_lang = lang.to_string();
                fence_content_lines = Vec::new();
                continue;
            } else if current_fence_char == fence_char && current_fence_len >= fence_len {
                let content_text = fence_content_lines.join("\n");
                let fp = fingerprint(&content_text);

                let lang_lower = fence_lang.to_lowercase();
                let lang_filter = language.map(|l| l.to_lowercase());
                let matches = lang_filter
                    .as_ref()
                    .map(|l| l == &lang_lower)
                    .unwrap_or(true);

                if matches {
                    blocks.push(CodeFenceBlock {
                        index,
                        language: fence_lang.clone(),
                        start_line: fence_start_line,
                        end_line: Some(line_num),
                        closed: true,
                        content: if include_content {
                            Some(content_text.clone())
                        } else {
                            None
                        },
                        fingerprint: fp,
                    });
                    index += 1;
                }

                in_fence = false;
                continue;
            }
        }

        if in_fence {
            fence_content_lines.push(line.to_string());
        }
    }

    if in_fence {
        let content_text = fence_content_lines.join("\n");
        let fp = fingerprint(&content_text);

        let lang_lower = fence_lang.to_lowercase();
        let lang_filter = language.map(|l| l.to_lowercase());
        let matches = lang_filter
            .as_ref()
            .map(|l| l == &lang_lower)
            .unwrap_or(true);

        let content_preview = if content_text.chars().count() > 200 {
            let truncated: String = content_text.chars().take(200).collect();
            truncated
        } else {
            content_text.clone()
        };

        unclosed_fences.push(serde_json::json!({
            "index": index,
            "language": fence_lang,
            "start_line": fence_start_line,
            "end_line": serde_json::Value::Null,
            "content_preview": content_preview,
            "fingerprint": fp,
        }));

        if matches {
            blocks.push(CodeFenceBlock {
                index,
                language: fence_lang.clone(),
                start_line: fence_start_line,
                end_line: None,
                closed: false,
                content: if include_content {
                    Some(content_text)
                } else {
                    None
                },
                fingerprint: fp,
            });
        }

        findings.push(format!(
            "Unclosed code fence starting at line {}",
            fence_start_line
        ));
    }

    CodeFenceExtractResult {
        blocks,
        unclosed_fences,
        findings,
    }
}
