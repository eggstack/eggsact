use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;

pub fn markdown_structure(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("markdown_structure"),
            )
        }
    };
    let include_sections = args
        .get("include_sections")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_links = args
        .get("include_links")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_code_fences = args
        .get("include_code_fences")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_html_comments = args
        .get("include_html_comments")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("markdown_structure"),
        );
    }

    let result = crate::text::markdown_structure(
        text,
        include_sections,
        include_links,
        include_code_fences,
        include_html_comments,
    );

    ToolResponse::success(
        serde_json::json!({
            "headings": result.headings,
            "code_fences": result.code_fences,
            "links": result.links,
            "html_comments": result.html_comments,
            "frontmatter": result.frontmatter,
            "tables_detected": result.tables_detected,
            "findings": result.findings,
        }),
        Some("markdown_structure"),
    )
    .with_tool("markdown_structure")
}

pub fn code_fence_extract(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("code_fence_extract"),
            )
        }
    };
    let language = args.get("language").and_then(|v| v.as_str());
    let include_content = args
        .get("include_content")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("code_fence_extract"),
        );
    }

    let result = crate::text::code_fence_extract(text, language, include_content);

    ToolResponse::success(
        serde_json::json!({
            "blocks": result.blocks,
            "unclosed_fences": result.unclosed_fences,
            "findings": result.findings,
        }),
        Some("code_fence_extract"),
    )
    .with_tool("code_fence_extract")
}
