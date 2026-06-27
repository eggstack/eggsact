use eggsact::text::{code_fence_extract, markdown_structure};

// ─── markdown_structure ──────────────────────────────────────────────

#[test]
fn test_markdown_structure_headings() {
    let text = "# Heading 1\n## Heading 2\n### Heading 3";
    let result = markdown_structure(text, true, false, false, false);
    assert_eq!(result.headings.len(), 3);
    assert_eq!(result.headings[0].level, 1);
    assert_eq!(result.headings[0].text, "Heading 1");
    assert_eq!(result.headings[1].level, 2);
    assert_eq!(result.headings[2].level, 3);
}

#[test]
fn test_markdown_structure_headings_with_slugs() {
    let text = "# Hello World";
    let result = markdown_structure(text, true, false, false, false);
    assert_eq!(result.headings.len(), 1);
    assert_eq!(result.headings[0].slug, "hello-world");
}

#[test]
fn test_markdown_structure_links() {
    let text = "[link](https://example.com)";
    let result = markdown_structure(text, false, true, false, false);
    assert_eq!(result.links.len(), 1);
    assert_eq!(result.links[0].visible_text, "link");
    assert_eq!(result.links[0].target, "https://example.com");
}

#[test]
fn test_markdown_structure_code_fences() {
    let text = "```python\nprint('hello')\n```";
    let result = markdown_structure(text, false, false, true, false);
    assert_eq!(result.code_fences.len(), 1);
    assert_eq!(result.code_fences[0].language, "python");
    assert!(result.code_fences[0].closed);
}

#[test]
fn test_markdown_structure_code_fence_no_language() {
    let text = "```\nhello\n```";
    let result = markdown_structure(text, false, false, true, false);
    assert_eq!(result.code_fences.len(), 1);
    assert!(result.code_fences[0].language.is_empty());
}

#[test]
fn test_markdown_structure_html_comments() {
    let text = "before\n<!-- comment -->\nafter";
    let result = markdown_structure(text, false, false, false, true);
    assert_eq!(result.html_comments.len(), 1);
    assert!(result.html_comments[0].text.contains("comment"));
}

#[test]
fn test_markdown_structure_frontmatter() {
    let text = "---\ntitle: test\n---\n# Hello";
    let result = markdown_structure(text, true, false, false, false);
    assert!(result.frontmatter.present);
}

#[test]
fn test_markdown_structure_tables() {
    let text = "| Name | Value |\n|------|-------|\n| a    | 1     |";
    let result = markdown_structure(text, false, false, false, false);
    assert!(result.tables_detected);
}

#[test]
fn test_markdown_structure_empty() {
    let result = markdown_structure("", false, false, false, false);
    assert!(result.headings.is_empty());
    assert!(result.links.is_empty());
}

#[test]
fn test_markdown_structure_no_headings() {
    let text = "Just some text\nwith no headings";
    let result = markdown_structure(text, true, false, false, false);
    assert!(result.headings.is_empty());
}

// ─── code_fence_extract ──────────────────────────────────────────────

#[test]
fn test_code_fence_extract_basic() {
    let text = "```python\nprint('hello')\n```";
    let result = code_fence_extract(text, None, true);
    assert_eq!(result.blocks.len(), 1);
    assert_eq!(result.blocks[0].language, "python");
    assert!(result.blocks[0].closed);
    assert!(result.blocks[0].content.is_some());
}

#[test]
fn test_code_fence_extract_multiple() {
    let text = "```python\nprint('hi')\n```\n\n```rust\nfn main() {}\n```";
    let result = code_fence_extract(text, None, true);
    assert_eq!(result.blocks.len(), 2);
}

#[test]
fn test_code_fence_extract_filter_language() {
    let text = "```python\nprint('hi')\n```\n\n```rust\nfn main() {}\n```";
    let result = code_fence_extract(text, Some("python"), true);
    assert_eq!(result.blocks.len(), 1);
    assert_eq!(result.blocks[0].language, "python");
}

#[test]
fn test_code_fence_extract_no_content() {
    let text = "```python\nprint('hello')\n```";
    let result = code_fence_extract(text, None, false);
    assert_eq!(result.blocks.len(), 1);
    assert!(result.blocks[0].content.is_none());
}

#[test]
fn test_code_fence_extract_unclosed() {
    let text = "```python\nprint('hello')";
    let result = code_fence_extract(text, None, true);
    assert!(!result.unclosed_fences.is_empty());
}

#[test]
fn test_code_fence_extract_empty() {
    let result = code_fence_extract("", None, false);
    assert!(result.blocks.is_empty());
}

#[test]
fn test_code_fence_extract_fingerprint() {
    let text = "```python\nprint('hello')\n```";
    let result = code_fence_extract(text, None, false);
    assert!(!result.blocks[0].fingerprint.is_empty());
}

#[test]
fn test_code_fence_extract_line_numbers() {
    let text = "line1\nline2\n```python\ncode\n```\nline5";
    let result = code_fence_extract(text, None, false);
    assert_eq!(result.blocks.len(), 1);
    assert_eq!(result.blocks[0].start_line, 3);
}

// ─── BUG-007: Long multi-byte CJK line not falsely flagged ─────────────

#[test]
fn test_markdown_structure_long_cjk_line_no_false_positive() {
    // 300 CJK characters (~3 bytes each in UTF-8), but only 300 chars
    // which is < 500 threshold for long minified line detection
    let cjk_char = '\u{4e2d}'; // 中
    let long_line: String = cjk_char.to_string().repeat(300);
    let text = format!("{}\n", long_line);

    let result = markdown_structure(&text, true, true, true, true);
    // Should not produce any findings (no false long-line flag)
    assert!(
        result.findings.is_empty(),
        "markdown_structure should not flag 300 CJK chars as long line: {:?}",
        result.findings
    );
    // The line should be processed without error
    assert!(!result.headings.is_empty() || result.headings.is_empty()); // No crash
}
