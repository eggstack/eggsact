use serde_json::Value;

pub fn markdown_structure_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Markdown text to analyze"},
            "include_sections": {"type": "boolean", "default": true, "description": "Include heading detection"},
            "include_links": {"type": "boolean", "default": true, "description": "Include link detection"},
            "include_code_fences": {"type": "boolean", "default": true, "description": "Include code fence detection"},
            "include_html_comments": {"type": "boolean", "default": true, "description": "Include HTML comment detection"}
        },
        "required": ["text"]
    })
}

pub fn code_fence_extract_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Markdown text to scan"},
            "language": {"type": "string", "description": "Optional language filter (case-insensitive)"},
            "include_content": {"type": "boolean", "default": true, "description": "Include block content in output"}
        },
        "required": ["text"]
    })
}

pub fn markdown_structure_output() -> Value {
    serde_json::json!({"type":"object","properties":{"headings":{"type":"array","description":"Headings with level, text, line, slug"},"code_fences":{"type":"array","description":"Code fences with language, lines, closed state"},"links":{"type":"array","description":"Links with visible text, target, mismatch flags"},"html_comments":{"type":"array","description":"HTML comments with text and position"},"frontmatter":{"type":"object","description":"Frontmatter detection (present, format, line range)"},"tables_detected":{"type":"boolean","description":"Whether Markdown tables were detected"},"findings":{"type":"array","items":{"type":"string"},"description":"Warnings and findings"}}})
}

pub fn code_fence_extract_output() -> Value {
    serde_json::json!({"type":"object","properties":{"blocks":{"type":"array","description":"Extracted code blocks with index, language, lines, content, fingerprint"},"unclosed_fences":{"type":"array","description":"Unclosed code fences found"},"findings":{"type":"array","items":{"type":"string"},"description":"Warnings and findings"}}})
}
