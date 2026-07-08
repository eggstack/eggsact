use crate::mcp::machine_codes;
use crate::mcp::schemas::{disposition, finding, severity, verdict, ToolResponse};
use crate::tools::helpers::*;
use serde_json::Value;

// ---------------------------------------------------------------------------
// import_export_inspect
// ---------------------------------------------------------------------------

pub fn import_export_inspect(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::CHEAP);

    let source = match _require_str(args, "source", "import_export_inspect") {
        Ok(s) => s,
        Err(resp) => return *resp,
    };

    if source.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "source length {} exceeds {}",
                source.chars().count(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some("import_export_inspect"),
        );
    }

    let language = args
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    let include_line_text = args
        .get("include_line_text")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_statements = args
        .get("max_statements")
        .and_then(|v| v.as_u64())
        .unwrap_or(500) as usize;

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("import_export_inspect")
            .unwrap_err();
    }

    let detected_language = if language == "auto" {
        detect_language_from_source(source)
    } else {
        language.to_string()
    };

    let statements = extract_imports_exports(source, &detected_language, include_line_text);
    let limited = apply_detail_limit(&statements, max_statements);
    let truncated = statements.len() > max_statements;

    let mut findings = Vec::new();
    let star_imports: Vec<&Value> = limited
        .iter()
        .filter(|s| {
            s.get("kind")
                .and_then(|v| v.as_str())
                .map(|k| k.contains("star") || k.contains("blank"))
                .unwrap_or(false)
        })
        .collect();
    for si in star_imports {
        findings.push(finding(
            machine_codes::SOURCE_INSPECT_HEURISTIC,
            severity::MEDIUM,
            &format!(
                "Wildcard/blank import detected at line {}",
                si.get("line").and_then(|v| v.as_u64()).unwrap_or(0)
            ),
            Some(disposition::CAUTION),
            None,
        ));
    }

    let response_verdict = if findings.iter().any(|f| {
        f.get("severity")
            .and_then(|s| s.as_str())
            .map(|s| s == "high" || s == "critical")
            .unwrap_or(false)
    }) {
        verdict::BLOCK
    } else if !findings.is_empty() {
        verdict::REVIEW
    } else {
        verdict::ALLOW
    };

    let machine_code = machine_codes::SOURCE_INSPECT_HEURISTIC;

    let result = serde_json::json!({
        "language": detected_language,
        "statements": limited,
        "statement_count": statements.len(),
        "truncated": truncated,
        "warnings": [],
        "limitations": detect_import_limitations(&detected_language),
    });

    let mut resp = ToolResponse::success(result, Some("import_export_inspect"))
        .with_tool("import_export_inspect")
        .with_machine_code(machine_code)
        .with_verdict(response_verdict);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

fn detect_language_from_source(source: &str) -> String {
    let trimmed = source.trim();
    if trimmed.contains("fn main()")
        || trimmed.contains("use std::")
        || trimmed.contains("use crate::")
        || trimmed.contains("pub fn ")
    {
        "rust".to_string()
    } else if trimmed.contains("def ") || trimmed.contains("import ") || trimmed.contains("from ") {
        if trimmed.contains("fn ") || trimmed.contains("const ") || trimmed.contains("export ") {
            "javascript".to_string()
        } else {
            "python".to_string()
        }
    } else if trimmed.contains("func ") || trimmed.contains("package ") {
        "go".to_string()
    } else if trimmed.contains("function ") || trimmed.contains("=>") {
        "javascript".to_string()
    } else {
        "unknown".to_string()
    }
}

fn extract_imports_exports(source: &str, language: &str, include_line_text: bool) -> Vec<Value> {
    let mut statements = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        match language {
            "rust" => extract_rust_imports(trimmed, line_num, include_line_text, &mut statements),
            "python" => {
                extract_python_imports(trimmed, line_num, include_line_text, &mut statements)
            }
            "javascript" | "typescript" => {
                extract_js_imports(trimmed, line_num, include_line_text, &mut statements)
            }
            "go" => extract_go_imports(trimmed, line_num, include_line_text, &mut statements),
            _ => {}
        }
    }
    statements
}

fn extract_rust_imports(
    line: &str,
    line_num: usize,
    include_line_text: bool,
    out: &mut Vec<Value>,
) {
    if line.starts_with("use ") || line.starts_with("pub use ") {
        let kind = if line.starts_with("pub use ") {
            "pub_use"
        } else {
            "use"
        };
        let module = line
            .trim_start_matches("pub ")
            .trim_start_matches("use ")
            .trim_end_matches(';')
            .trim()
            .to_string();
        let mut entry = serde_json::json!({
            "kind": kind,
            "module": module,
            "line": line_num,
            "confidence": "high",
        });
        if include_line_text {
            entry["raw_text"] = serde_json::json!(line);
        }
        out.push(entry);
    } else if line.starts_with("mod ") {
        let module = line
            .trim_start_matches("mod ")
            .trim_end_matches(';')
            .trim()
            .to_string();
        let mut entry = serde_json::json!({
            "kind": "mod",
            "module": module,
            "line": line_num,
            "confidence": "high",
        });
        if include_line_text {
            entry["raw_text"] = serde_json::json!(line);
        }
        out.push(entry);
    } else if line.starts_with("extern crate ") {
        let module = line
            .trim_start_matches("extern crate ")
            .trim_end_matches(';')
            .trim()
            .to_string();
        let mut entry = serde_json::json!({
            "kind": "extern_crate",
            "module": module,
            "line": line_num,
            "confidence": "high",
        });
        if include_line_text {
            entry["raw_text"] = serde_json::json!(line);
        }
        out.push(entry);
    }
}

fn extract_python_imports(
    line: &str,
    line_num: usize,
    include_line_text: bool,
    out: &mut Vec<Value>,
) {
    if line.starts_with("from ") && line.contains(" import ") {
        let parts: Vec<&str> = line.splitn(2, " import ").collect();
        let module = parts[0].trim_start_matches("from ").trim().to_string();
        let symbols_part = parts.get(1).unwrap_or(&"").trim_end_matches(':');
        let kind = if symbols_part.contains('*') {
            "from_import_star"
        } else {
            "from_import"
        };
        let mut entry = serde_json::json!({
            "kind": kind,
            "module": module,
            "symbols": symbols_part,
            "line": line_num,
            "confidence": "high",
        });
        if include_line_text {
            entry["raw_text"] = serde_json::json!(line);
        }
        out.push(entry);
    } else if line.starts_with("import ") {
        let module = line
            .trim_start_matches("import ")
            .trim_end_matches(':')
            .trim()
            .to_string();
        let kind = if line.contains(" as ") {
            "import_alias"
        } else {
            "import"
        };
        let mut entry = serde_json::json!({
            "kind": kind,
            "module": module,
            "line": line_num,
            "confidence": "high",
        });
        if include_line_text {
            entry["raw_text"] = serde_json::json!(line);
        }
        out.push(entry);
    }
}

fn extract_js_imports(line: &str, line_num: usize, include_line_text: bool, out: &mut Vec<Value>) {
    if line.starts_with("import ") {
        if line.contains(" from ") {
            let parts: Vec<&str> = line.splitn(2, " from ").collect();
            let symbols = parts[0].trim_start_matches("import ").trim().to_string();
            let module = parts
                .get(1)
                .unwrap_or(&"")
                .trim()
                .trim_end_matches(';')
                .trim_matches('\'')
                .trim_matches('"')
                .to_string();
            let mut entry = serde_json::json!({
                "kind": "import_from",
                "module": module,
                "symbols": symbols,
                "line": line_num,
                "confidence": "high",
            });
            if include_line_text {
                entry["raw_text"] = serde_json::json!(line);
            }
            out.push(entry);
        } else {
            let module = line
                .trim_start_matches("import ")
                .trim_end_matches(';')
                .trim()
                .trim_matches('\'')
                .trim_matches('"')
                .to_string();
            let mut entry = serde_json::json!({
                "kind": "import_side_effect",
                "module": module,
                "line": line_num,
                "confidence": "medium",
            });
            if include_line_text {
                entry["raw_text"] = serde_json::json!(line);
            }
            out.push(entry);
        }
    } else if line.starts_with("export ") {
        let kind = if line.starts_with("export default ") {
            "export_default"
        } else if line.starts_with("export async ") || line.starts_with("export function ") {
            "export_function"
        } else if line.starts_with("export class ") {
            "export_class"
        } else if line.starts_with("export const ")
            || line.starts_with("export let ")
            || line.starts_with("export var ")
        {
            "export_variable"
        } else {
            "export"
        };
        let mut entry = serde_json::json!({
            "kind": kind,
            "line": line_num,
            "confidence": "high",
        });
        if include_line_text {
            entry["raw_text"] = serde_json::json!(line);
        }
        out.push(entry);
    } else if line.starts_with("const ") && line.contains("= require(") {
        let module = line
            .split_once("= require(")
            .map(|(_, right)| {
                right
                    .trim_end_matches(')')
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string()
            })
            .unwrap_or_default();
        let mut entry = serde_json::json!({
            "kind": "require",
            "module": module,
            "line": line_num,
            "confidence": "medium",
        });
        if include_line_text {
            entry["raw_text"] = serde_json::json!(line);
        }
        out.push(entry);
    }
}

fn extract_go_imports(line: &str, line_num: usize, include_line_text: bool, out: &mut Vec<Value>) {
    if line.starts_with("import ") {
        if line.contains("(") {
            // Import block start - skip, we handle single-line imports
        } else {
            let module = line
                .trim_start_matches("import ")
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim()
                .to_string();
            let kind = if module.starts_with('_') {
                "import_blank"
            } else if module.contains(" as ") {
                "import_alias"
            } else {
                "import"
            };
            let mut entry = serde_json::json!({
                "kind": kind,
                "module": module,
                "line": line_num,
                "confidence": "high",
            });
            if include_line_text {
                entry["raw_text"] = serde_json::json!(line);
            }
            out.push(entry);
        }
    }
}

fn detect_import_limitations(language: &str) -> Vec<String> {
    let mut limitations = Vec::new();
    match language {
        "rust" => {
            limitations.push("Does not resolve nested use paths or glob imports".to_string());
        }
        "python" => {
            limitations.push(
                "Does not parse multi-line import statements or conditional imports".to_string(),
            );
        }
        "javascript" | "typescript" => {
            limitations.push("Does not handle dynamic import() expressions".to_string());
            limitations.push("TypeScript-specific import syntax not distinguished".to_string());
        }
        "go" => {
            limitations.push("Does not parse multi-line import blocks".to_string());
        }
        _ => {
            limitations.push("Language not recognized; no statements extracted".to_string());
        }
    }
    limitations
}

// ---------------------------------------------------------------------------
// code_block_map
// ---------------------------------------------------------------------------

pub fn code_block_map(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::CHEAP);

    let source = match _require_str(args, "source", "code_block_map") {
        Ok(s) => s,
        Err(resp) => return *resp,
    };

    if source.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "source length {} exceeds {}",
                source.chars().count(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some("code_block_map"),
        );
    }

    let language = args
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    let max_blocks = args
        .get("max_blocks")
        .and_then(|v| v.as_u64())
        .unwrap_or(500) as usize;
    let include_nested = args
        .get("include_nested")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if budget_ctx.should_stop() {
        return budget_ctx.check_should_stop("code_block_map").unwrap_err();
    }

    let detected_language = if language == "auto" {
        detect_language_from_source(source)
    } else {
        language.to_string()
    };

    let blocks = extract_blocks(source, &detected_language, include_nested);
    let limited = apply_detail_limit(&blocks, max_blocks);
    let truncated = blocks.len() > max_blocks;

    let mut findings = Vec::new();
    if detected_language == "unknown" {
        findings.push(finding(
            machine_codes::SOURCE_INSPECT_HEURISTIC,
            severity::LOW,
            "Could not auto-detect language; no blocks extracted",
            Some(disposition::INFORMATIONAL),
            None,
        ));
    }

    let response_verdict = if detected_language == "unknown" {
        verdict::REVIEW
    } else {
        verdict::ALLOW
    };

    let result = serde_json::json!({
        "language": detected_language,
        "blocks": limited,
        "block_count": blocks.len(),
        "truncated": truncated,
        "warnings": [],
        "limitations": detect_block_limitations(&detected_language),
    });

    let mut resp = ToolResponse::success(result, Some("code_block_map"))
        .with_tool("code_block_map")
        .with_machine_code(machine_codes::SOURCE_INSPECT_HEURISTIC)
        .with_verdict(response_verdict);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

fn extract_blocks(source: &str, language: &str, include_nested: bool) -> Vec<Value> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    match language {
        "rust" | "javascript" | "typescript" => {
            extract_brace_blocks(&lines, language, include_nested, &mut blocks);
        }
        "python" => {
            extract_python_blocks(&lines, include_nested, &mut blocks);
        }
        "go" => {
            extract_brace_blocks(&lines, "go", include_nested, &mut blocks);
        }
        "markdown" => {
            extract_markdown_blocks(&lines, &mut blocks);
        }
        _ => {}
    }
    blocks
}

fn extract_brace_blocks(
    lines: &[&str],
    language: &str,
    include_nested: bool,
    out: &mut Vec<Value>,
) {
    let mut depth = 0i32;
    let mut block_start: Option<usize> = None;
    let mut current_sig = String::new();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if depth == 0 {
            // Look for block-starting signatures
            let sig = detect_brace_signature(trimmed, language);
            if let Some((kind, name)) = sig {
                block_start = Some(idx);
                current_sig = format!("{} {}", kind, name);
            }
        }

        let open = trimmed.bytes().filter(|&b| b == b'{').count() as i32;
        let close = trimmed.bytes().filter(|&b| b == b'}').count() as i32;
        depth += open - close;

        if depth <= 0 && block_start.is_some() {
            let start = block_start.unwrap() + 1;
            let end = idx + 1;
            if include_nested || depth == 0 && open == 0 && close > 0 || block_start.is_some() {
                let parts: Vec<&str> = current_sig.splitn(2, ' ').collect();
                let kind = parts.first().unwrap_or(&"").to_string();
                let name = parts.get(1).unwrap_or(&"").to_string();
                out.push(serde_json::json!({
                    "kind": kind,
                    "name": name,
                    "start_line": start,
                    "end_line": end,
                    "confidence": "medium",
                    "raw_signature": current_sig,
                }));
            }
            block_start = None;
            current_sig.clear();
            if depth < 0 {
                depth = 0;
            }
        }
    }
}

fn detect_brace_signature<'a>(line: &'a str, language: &str) -> Option<(&'a str, String)> {
    if line.is_empty() || line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
    {
        return None;
    }

    match language {
        "rust" => {
            if line.starts_with("fn ")
                || line.starts_with("pub fn ")
                || line.starts_with("pub(crate) fn ")
            {
                let name = line
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("pub(crate) ")
                    .trim_start_matches("pub ")
                    .trim_start_matches("fn ")
                    .to_string();
                return Some(("fn", name));
            }
            if line.starts_with("struct ") || line.starts_with("pub struct ") {
                let name = line
                    .split('{')
                    .next()
                    .or_else(|| line.split(';').next())
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("pub ")
                    .trim_start_matches("struct ")
                    .trim()
                    .to_string();
                return Some(("struct", name));
            }
            if line.starts_with("enum ") || line.starts_with("pub enum ") {
                let name = line
                    .split('{')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("pub ")
                    .trim_start_matches("enum ")
                    .trim()
                    .to_string();
                return Some(("enum", name));
            }
            if line.starts_with("impl ") || line.starts_with("pub impl ") {
                let name = line
                    .split('{')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("pub ")
                    .trim_start_matches("impl ")
                    .trim()
                    .to_string();
                return Some(("impl", name));
            }
            if line.starts_with("trait ") || line.starts_with("pub trait ") {
                let name = line
                    .split('{')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("pub ")
                    .trim_start_matches("trait ")
                    .trim()
                    .to_string();
                return Some(("trait", name));
            }
            if line.starts_with("mod ") || line.starts_with("pub mod ") {
                let name = line
                    .split('{')
                    .next()
                    .or_else(|| line.split(';').next())
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("pub ")
                    .trim_start_matches("mod ")
                    .trim()
                    .to_string();
                return Some(("mod", name));
            }
        }
        "javascript" | "typescript" => {
            if line.starts_with("function ") || line.starts_with("async function ") {
                let name = line
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("async ")
                    .trim_start_matches("function ")
                    .to_string();
                return Some(("function", name));
            }
            if line.starts_with("class ") || line.starts_with("export class ") {
                let name = line
                    .split('{')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("export ")
                    .trim_start_matches("class ")
                    .to_string();
                return Some(("class", name));
            }
            if line.starts_with("const ") && (line.contains("= (") || line.contains("= async (")) {
                let name = line
                    .split('=')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("const ")
                    .to_string();
                return Some(("arrow_function", name));
            }
        }
        "go" => {
            if line.starts_with("func ") {
                let name = line
                    .split('{')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("func ")
                    .to_string();
                return Some(("func", name));
            }
            if line.starts_with("type ") {
                let name = line
                    .split('{')
                    .next()
                    .or_else(|| line.split('=').next())
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("type ")
                    .to_string();
                return Some(("type", name));
            }
        }
        _ => {}
    }
    None
}

fn extract_python_blocks(lines: &[&str], include_nested: bool, out: &mut Vec<Value>) {
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let kind_name = if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            let is_async = trimmed.starts_with("async ");
            let sig = trimmed
                .trim_start_matches("async ")
                .trim_start_matches("def ");
            let name = sig.split('(').next().unwrap_or("").trim().to_string();
            let kind = if is_async { "async_def" } else { "def" };
            Some((kind, name))
        } else if trimmed.starts_with("class ") {
            let name = trimmed
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                .trim_start_matches("class ")
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            Some(("class", name))
        } else {
            None
        };

        if let Some((kind, name)) = kind_name {
            // Find block end by indentation
            let base_indent = line.len() - line.trim_start().len();
            let mut end_line = idx;
            for (subsequent_idx, sub_line) in lines.iter().enumerate().skip(idx + 1) {
                if sub_line.trim().is_empty() {
                    end_line = subsequent_idx;
                    continue;
                }
                let sub_indent = sub_line.len() - sub_line.trim_start().len();
                if sub_indent <= base_indent {
                    break;
                }
                end_line = subsequent_idx;
                if !include_nested {
                    // Only include top-level blocks (indent 0)
                    if base_indent > 0 {
                        break;
                    }
                }
            }
            out.push(serde_json::json!({
                "kind": kind,
                "name": name,
                "start_line": idx + 1,
                "end_line": end_line + 1,
                "confidence": "high",
                "raw_signature": trimmed.split(':').next().unwrap_or("").trim(),
            }));
        }
    }
}

fn extract_markdown_blocks(lines: &[&str], out: &mut Vec<Value>) {
    let mut in_fence = false;
    let mut fence_start = 0;
    let mut fence_lang = String::new();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if !in_fence {
                in_fence = true;
                fence_start = idx;
                fence_lang = trimmed.trim_start_matches('`').trim().to_string();
            } else {
                let kind = if fence_lang.is_empty() {
                    "code_block"
                } else {
                    "fenced_code"
                };
                out.push(serde_json::json!({
                    "kind": kind,
                    "name": fence_lang,
                    "start_line": fence_start + 1,
                    "end_line": idx + 1,
                    "confidence": "high",
                    "raw_signature": format!("```{}", fence_lang),
                }));
                in_fence = false;
                fence_lang.clear();
            }
        } else if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let name = trimmed.trim_start_matches('#').trim().to_string();
            out.push(serde_json::json!({
                "kind": "heading",
                "name": name,
                "start_line": idx + 1,
                "end_line": idx + 1,
                "confidence": "high",
                "raw_signature": trimmed,
            }));
            // Store level for potential future use
            let _ = level;
        }
    }
}

fn detect_block_limitations(language: &str) -> Vec<String> {
    let mut limitations = Vec::new();
    match language {
        "rust" => {
            limitations
                .push("Brace-based detection; does not handle macros or attributes".to_string());
        }
        "python" => {
            limitations.push(
                "Indentation-based; may misclassify nested blocks at non-standard indentation"
                    .to_string(),
            );
        }
        "javascript" | "typescript" => {
            limitations
                .push("Brace-based; does not handle arrow functions without braces".to_string());
        }
        "go" => {
            limitations.push("Brace-based; method receivers included in name".to_string());
        }
        "markdown" => {
            limitations.push("Heading and fenced-code-block detection only".to_string());
        }
        _ => {}
    }
    limitations
}

// ---------------------------------------------------------------------------
// symbol_name_diff
// ---------------------------------------------------------------------------

pub fn symbol_name_diff(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::CHEAP);

    let old_source = match _require_str(args, "old_source", "symbol_name_diff") {
        Ok(s) => s,
        Err(resp) => return *resp,
    };
    let new_source = match _require_str(args, "new_source", "symbol_name_diff") {
        Ok(s) => s,
        Err(resp) => return *resp,
    };

    if old_source.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "old_source length {} exceeds {}",
                old_source.chars().count(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some("symbol_name_diff"),
        );
    }
    if new_source.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "new_source length {} exceeds {}",
                new_source.chars().count(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some("symbol_name_diff"),
        );
    }

    let language = args
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    let rename_threshold = args
        .get("rename_similarity_threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.6);

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("symbol_name_diff")
            .unwrap_err();
    }

    let detected_language = if language == "auto" {
        detect_language_from_source(old_source)
    } else {
        language.to_string()
    };

    let old_blocks = extract_blocks(old_source, &detected_language, false);
    let new_blocks = extract_blocks(new_source, &detected_language, false);

    let old_names: Vec<String> = old_blocks
        .iter()
        .filter_map(|b| b.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();
    let new_names: Vec<String> = new_blocks
        .iter()
        .filter_map(|b| b.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();

    let old_set: std::collections::HashSet<&String> = old_names.iter().collect();
    let new_set: std::collections::HashSet<&String> = new_names.iter().collect();

    let added: Vec<String> = new_names
        .iter()
        .filter(|n| !old_set.contains(n))
        .cloned()
        .collect();
    let removed: Vec<String> = old_names
        .iter()
        .filter(|n| !new_set.contains(n))
        .cloned()
        .collect();
    let unchanged: Vec<String> = old_names
        .iter()
        .filter(|n| new_set.contains(n))
        .cloned()
        .collect();

    // Detect possible renames by name similarity
    let mut possible_renames = Vec::new();
    for r_name in &removed {
        for a_name in &added {
            let sim = name_similarity(r_name, a_name);
            if sim >= rename_threshold {
                possible_renames.push(serde_json::json!({
                    "old": r_name,
                    "new": a_name,
                    "confidence": format!("{:.2}", sim),
                }));
            }
        }
    }

    let mut warnings = Vec::new();
    if detected_language == "unknown" {
        warnings.push("Language not recognized; symbol detection may be inaccurate".to_string());
    }

    let result = serde_json::json!({
        "language": detected_language,
        "added": added,
        "removed": removed,
        "unchanged": unchanged,
        "possible_renames": possible_renames,
        "added_count": added.len(),
        "removed_count": removed.len(),
        "unchanged_count": unchanged.len(),
        "warnings": warnings,
        "limitations": vec![
            "Brace/indentation-based heuristic only; not a full parser".to_string(),
            "Rename detection uses name similarity, not semantic analysis".to_string(),
        ],
    });

    let mut findings = Vec::new();
    if !added.is_empty() || !removed.is_empty() {
        findings.push(finding(
            machine_codes::SOURCE_INSPECT_HEURISTIC,
            severity::INFO,
            &format!(
                "{} added, {} removed, {} possible renames",
                added.len(),
                removed.len(),
                possible_renames.len()
            ),
            Some(disposition::INFORMATIONAL),
            None,
        ));
    }

    ToolResponse::success(result, Some("symbol_name_diff"))
        .with_tool("symbol_name_diff")
        .with_machine_code(machine_codes::SOURCE_INSPECT_HEURISTIC)
        .with_verdict(verdict::ALLOW)
        .with_findings(findings)
}

/// Simple name similarity based on longest common subsequence ratio.
fn name_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    if a_lower == b_lower {
        return 1.0;
    }
    let a_bytes = a_lower.as_bytes();
    let b_bytes = b_lower.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    if m == 0 || n == 0 {
        return 0.0;
    }
    // LCS length
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if a_bytes[i - 1] == b_bytes[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }
    let lcs_len = dp[m][n] as f64;
    let max_len = m.max(n) as f64;
    lcs_len / max_len
}

// ---------------------------------------------------------------------------
// lockfile_inspect
// ---------------------------------------------------------------------------

pub fn lockfile_inspect(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::MODERATE);

    let path = args.get("path").and_then(|v| v.as_str());
    let before = args.get("before").and_then(|v| v.as_str());
    let after = args.get("after").and_then(|v| v.as_str());
    let diff = args.get("diff").and_then(|v| v.as_str());
    let ecosystem = args
        .get("ecosystem")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    let max_packages = args
        .get("max_packages")
        .and_then(|v| v.as_u64())
        .unwrap_or(500) as usize;

    // Validate inputs
    if before.is_none() && after.is_none() && diff.is_none() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "At least one of 'before', 'after', or 'diff' must be provided",
            None,
            Some("lockfile_inspect"),
        );
    }

    if let Some(b) = before {
        if b.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INPUT_TOO_LARGE,
                &format!(
                    "before text length {} exceeds {}",
                    b.chars().count(),
                    MAX_TEXT_LENGTH
                ),
                None,
                Some("lockfile_inspect"),
            );
        }
    }
    if let Some(a) = after {
        if a.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INPUT_TOO_LARGE,
                &format!(
                    "after text length {} exceeds {}",
                    a.chars().count(),
                    MAX_TEXT_LENGTH
                ),
                None,
                Some("lockfile_inspect"),
            );
        }
    }
    if let Some(d) = diff {
        if d.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INPUT_TOO_LARGE,
                &format!(
                    "diff text length {} exceeds {}",
                    d.chars().count(),
                    MAX_TEXT_LENGTH
                ),
                None,
                Some("lockfile_inspect"),
            );
        }
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("lockfile_inspect")
            .unwrap_err();
    }

    let detected_ecosystem = if ecosystem == "auto" {
        detect_lockfile_ecosystem(path, before, after)
    } else {
        ecosystem.to_string()
    };

    let changes = analyze_lockfile_changes(before, after, diff, &detected_ecosystem, max_packages);
    let mut findings = Vec::new();

    for change in &changes {
        if let Some(change_type) = change.get("type").and_then(|v| v.as_str()) {
            match change_type {
                "added" | "removed" => {
                    findings.push(finding(
                        machine_codes::LOCKFILE_CHANGE_DETECTED,
                        severity::MEDIUM,
                        &format!(
                            "{} package: {}",
                            change_type,
                            change
                                .get("package")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                        ),
                        Some(disposition::CAUTION),
                        None,
                    ));
                }
                "updated" => {
                    findings.push(finding(
                        machine_codes::LOCKFILE_CHANGE_DETECTED,
                        severity::LOW,
                        &format!(
                            "Updated package: {}",
                            change
                                .get("package")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                        ),
                        Some(disposition::INFORMATIONAL),
                        None,
                    ));
                }
                _ => {}
            }
        }
    }

    let has_git_or_path = changes.iter().any(|c| {
        c.get("source")
            .and_then(|v| v.as_str())
            .map(|s| s.starts_with("git+") || s.starts_with("path:"))
            .unwrap_or(false)
    });
    if has_git_or_path {
        findings.push(finding(
            machine_codes::LOCKFILE_CHANGE_DETECTED,
            severity::HIGH,
            "Git or path dependencies detected in lockfile",
            Some(disposition::BLOCKING),
            None,
        ));
    }

    let response_verdict = if findings.iter().any(|f| {
        f.get("severity")
            .and_then(|s| s.as_str())
            .map(|s| s == "high" || s == "critical")
            .unwrap_or(false)
    }) {
        verdict::BLOCK
    } else if !findings.is_empty() {
        verdict::REVIEW
    } else {
        verdict::ALLOW
    };

    let machine_code = machine_codes::LOCKFILE_CHANGE_DETECTED;

    let result = serde_json::json!({
        "ecosystem": detected_ecosystem,
        "changes": changes,
        "change_count": changes.len(),
        "large_churn": changes.len() > 50,
        "summary": format!("{} changes detected in lockfile", changes.len()),
        "limitations": detect_lockfile_limitations(&detected_ecosystem),
    });

    let mut resp = ToolResponse::success(result, Some("lockfile_inspect"))
        .with_tool("lockfile_inspect")
        .with_machine_code(machine_code)
        .with_verdict(response_verdict);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

fn detect_lockfile_ecosystem(
    path: Option<&str>,
    before: Option<&str>,
    after: Option<&str>,
) -> String {
    if let Some(p) = path {
        let lower = p.to_lowercase();
        if lower.ends_with("cargo.lock") {
            return "cargo".to_string();
        }
        if lower.ends_with("package-lock.json") {
            return "npm".to_string();
        }
        if lower.ends_with("pnpm-lock.yaml") {
            return "pnpm".to_string();
        }
        if lower.ends_with("yarn.lock") {
            return "yarn".to_string();
        }
        if lower.ends_with("poetry.lock") {
            return "poetry".to_string();
        }
        if lower.ends_with("go.sum") {
            return "go".to_string();
        }
        if lower.ends_with("uv.lock") {
            return "uv".to_string();
        }
    }
    // Content-based detection
    let sample = after.or(before).unwrap_or("");
    if sample.contains("[[package]]") && sample.contains("name = ") {
        return "cargo".to_string();
    }
    if sample.contains("\"dependencies\"") || sample.contains("\"packages\"") {
        return "npm".to_string();
    }
    if sample.contains("specifiers:") || sample.contains("resolution:") {
        return "pnpm".to_string();
    }
    if sample.starts_with('#') && sample.contains("resolved") {
        return "yarn".to_string();
    }
    if sample.contains("content-hash") || sample.contains("[[package]]") {
        return "poetry".to_string();
    }
    if sample.starts_with("module ") || sample.contains("go\t") {
        return "go".to_string();
    }
    "unknown".to_string()
}

fn analyze_lockfile_changes(
    before: Option<&str>,
    after: Option<&str>,
    diff: Option<&str>,
    ecosystem: &str,
    max_packages: usize,
) -> Vec<Value> {
    let mut changes = Vec::new();

    // If we have a diff, parse it for added/removed lines
    if let Some(d) = diff {
        for line in d.lines().take(max_packages) {
            let trimmed = line.trim();
            if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
                let content = &trimmed[1..];
                if let Some(pkg) = extract_package_from_line(content, ecosystem) {
                    changes.push(serde_json::json!({
                        "type": "added",
                        "package": pkg.name,
                        "version": pkg.version,
                        "source": pkg.source,
                    }));
                }
            } else if trimmed.starts_with('-') && !trimmed.starts_with("---") {
                let content = &trimmed[1..];
                if let Some(pkg) = extract_package_from_line(content, ecosystem) {
                    changes.push(serde_json::json!({
                        "type": "removed",
                        "package": pkg.name,
                        "version": pkg.version,
                        "source": pkg.source,
                    }));
                }
            }
        }
        return changes;
    }

    // If we have before/after, extract packages from both and diff
    if let (Some(b), Some(a)) = (before, after) {
        let before_pkgs = extract_packages(b, ecosystem, max_packages);
        let after_pkgs = extract_packages(a, ecosystem, max_packages);

        let before_map: std::collections::HashMap<String, &LockfilePackage> =
            before_pkgs.iter().map(|p| (p.name.clone(), p)).collect();
        let after_map: std::collections::HashMap<String, &LockfilePackage> =
            after_pkgs.iter().map(|p| (p.name.clone(), p)).collect();

        for (name, pkg) in &after_map {
            if let Some(old_pkg) = before_map.get(name) {
                if old_pkg.version != pkg.version || old_pkg.source != pkg.source {
                    changes.push(serde_json::json!({
                        "type": "updated",
                        "package": name,
                        "old_version": old_pkg.version,
                        "new_version": pkg.version,
                        "old_source": old_pkg.source,
                        "new_source": pkg.source,
                    }));
                }
            } else {
                changes.push(serde_json::json!({
                    "type": "added",
                    "package": name,
                    "version": pkg.version,
                    "source": pkg.source,
                }));
            }
        }
        for (name, pkg) in &before_map {
            if !after_map.contains_key(name) {
                changes.push(serde_json::json!({
                    "type": "removed",
                    "package": name,
                    "version": pkg.version,
                    "source": pkg.source,
                }));
            }
        }
    }

    changes
}

struct LockfilePackage {
    name: String,
    version: String,
    source: String,
}

fn extract_package_from_line(line: &str, ecosystem: &str) -> Option<LockfilePackage> {
    match ecosystem {
        "cargo" => {
            // Line like: name = "foo" or version = "1.0.0"
            // We need to collect name+version pairs, but for a single line
            // we can only detect if it looks like a package entry
            if line.contains("name = ") {
                let name = line
                    .split("name = ")
                    .nth(1)?
                    .trim()
                    .trim_matches('"')
                    .to_string();
                Some(LockfilePackage {
                    name,
                    version: String::new(),
                    source: String::new(),
                })
            } else {
                None
            }
        }
        "npm" => {
            // JSON lines like: "package-name": { "version": "1.0.0" }
            if line.contains("\"version\"") {
                let name = line.split('"').nth(1)?.to_string();
                let version = line.split("version").nth(1)?.split('"').nth(2)?.to_string();
                Some(LockfilePackage {
                    name,
                    version,
                    source: String::new(),
                })
            } else {
                None
            }
        }
        "go" => {
            // Lines like: github.com/foo/bar v1.0.0 h1:...
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(LockfilePackage {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    source: String::new(),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_packages(content: &str, ecosystem: &str, max_packages: usize) -> Vec<LockfilePackage> {
    let mut packages = Vec::new();
    match ecosystem {
        "cargo" => {
            let mut current_name = String::new();
            let mut current_version = String::new();
            let mut current_source = String::new();
            for line in content.lines().take(max_packages * 4) {
                let trimmed = line.trim();
                if trimmed.starts_with("name = ") {
                    current_name = trimmed
                        .split("name = ")
                        .nth(1)
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_string();
                } else if trimmed.starts_with("version = ") {
                    current_version = trimmed
                        .split("version = ")
                        .nth(1)
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_string();
                } else if trimmed.starts_with("source = ") {
                    current_source = trimmed
                        .split("source = ")
                        .nth(1)
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_string();
                } else if (trimmed == "[[package]]" || trimmed.is_empty())
                    && !current_name.is_empty()
                {
                    packages.push(LockfilePackage {
                        name: current_name.clone(),
                        version: current_version.clone(),
                        source: current_source.clone(),
                    });
                    current_name.clear();
                    current_version.clear();
                    current_source.clear();
                    if packages.len() >= max_packages {
                        break;
                    }
                }
            }
            if !current_name.is_empty() && packages.len() < max_packages {
                packages.push(LockfilePackage {
                    name: current_name,
                    version: current_version,
                    source: current_source,
                });
            }
        }
        "npm" => {
            // Simple line-based extraction for npm
            if let Ok(val) = serde_json::from_str::<Value>(content) {
                // Try "packages" key (npm v2+ lockfile)
                if let Some(pkgs) = val.get("packages").and_then(|v| v.as_object()) {
                    for (path, info) in pkgs.iter().take(max_packages) {
                        if path.is_empty() {
                            continue;
                        }
                        let name = path.rsplit('/').next().unwrap_or(path).to_string();
                        let version = info
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        packages.push(LockfilePackage {
                            name,
                            version,
                            source: String::new(),
                        });
                    }
                }
                // Try "dependencies" key (npm v1 lockfile)
                if let Some(deps) = val.get("dependencies").and_then(|v| v.as_object()) {
                    for (name, info) in deps.iter().take(max_packages) {
                        let version = info
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        packages.push(LockfilePackage {
                            name: name.clone(),
                            version,
                            source: String::new(),
                        });
                    }
                }
            }
        }
        "go" => {
            for line in content.lines().take(max_packages) {
                if let Some(pkg) = extract_package_from_line(line, "go") {
                    packages.push(pkg);
                    if packages.len() >= max_packages {
                        break;
                    }
                }
            }
        }
        _ => {
            // Heuristic: look for lines with package-like patterns
            for line in content.lines().take(max_packages) {
                let trimmed = line.trim();
                if trimmed.contains("version") || trimmed.contains('@') {
                    packages.push(LockfilePackage {
                        name: trimmed.to_string(),
                        version: String::new(),
                        source: String::new(),
                    });
                    if packages.len() >= max_packages {
                        break;
                    }
                }
            }
        }
    }
    packages
}

fn detect_lockfile_limitations(ecosystem: &str) -> Vec<String> {
    let mut limitations = Vec::new();
    match ecosystem {
        "cargo" => {
            limitations.push(
                "Line-based TOML extraction; does not fully parse nested structures".to_string(),
            );
        }
        "npm" => {
            limitations.push(
                "Parses JSON lockfile but may miss transitive dependency metadata".to_string(),
            );
        }
        "pnpm" | "yarn" => {
            limitations.push(
                "Heuristic line-based detection; no YAML/lockfile parser available".to_string(),
            );
        }
        "go" => {
            limitations
                .push("Line-based extraction from go.sum; does not verify checksums".to_string());
        }
        _ => {
            limitations.push("Unknown lockfile ecosystem; limited analysis possible".to_string());
        }
    }
    limitations.push("Does not query package registries or check for vulnerabilities".to_string());
    limitations
}
