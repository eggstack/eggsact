use eggsact::text::{code_fence_extract, markdown_structure};

#[test]
fn markdown_structure_no_panic() {
    let inputs = ["", "# Hello", "```rust\ncode\n```", "text\n\nmore text"];
    for input in &inputs {
        let _ = markdown_structure(input, true, true, true, true);
    }
}

#[test]
fn markdown_structure_deterministic() {
    let inputs = ["# Hello\n\n```rust\ncode\n```", "", "plain text"];
    for input in &inputs {
        let s1 = markdown_structure(input, true, true, true, true);
        let s2 = markdown_structure(input, true, true, true, true);
        assert_eq!(s1.headings.len(), s2.headings.len());
        assert_eq!(s1.code_fences.len(), s2.code_fences.len());
    }
}

#[test]
fn code_fence_extract_no_panic() {
    let inputs = [
        "```rust\ncode\n```",
        "```\ncode\n```",
        "unclosed fence",
        "```python\nprint('hi')",
    ];
    for input in &inputs {
        let _ = code_fence_extract(input, None, true);
        let _ = code_fence_extract(input, Some("rust"), false);
    }
}

#[test]
fn code_fence_spans_ordered() {
    let input = "text\n```rust\ncode1\n```\nmore\n```python\ncode2\n```\nend";
    let result = code_fence_extract(input, None, true);
    let mut prev_end = 0;
    for block in &result.blocks {
        assert!(block.start_line >= prev_end);
        if let Some(end) = block.end_line {
            prev_end = end;
        }
    }
}

#[test]
fn code_fence_deterministic() {
    let input = "text\n```rust\ncode\n```\nmore";
    let r1 = code_fence_extract(input, None, true);
    let r2 = code_fence_extract(input, None, true);
    assert_eq!(r1.blocks.len(), r2.blocks.len());
}
