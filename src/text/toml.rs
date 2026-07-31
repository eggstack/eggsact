use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item};

const MAX_INPUT_LENGTH: usize = 100_000;

fn byte_offset_to_line_col(text: &str, offset: usize) -> (i32, i32) {
    let mut line = 1i32;
    let mut col = 1i32;
    let mut byte_pos = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if byte_pos >= offset {
            break;
        }
        let char_len = c.len_utf8();
        byte_pos += char_len;
        if c == '\r' {
            if byte_pos < text.len() && text.as_bytes()[byte_pos] == b'\n' {
                byte_pos += 1;
                chars.next();
            }
            line += 1;
            col = 1;
        } else if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateTomlResult {
    pub valid: bool,
    pub error: Option<String>,
    pub line: Option<i32>,
    pub column: Option<i32>,
    pub position: Option<i32>,
    #[serde(rename = "type")]
    pub toml_type: Option<String>,
    pub top_level_keys: Option<Vec<String>>,
    pub tables: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TomlShapeResult {
    pub valid: bool,
    pub top_level_keys: Option<Vec<String>>,
    pub tables: Option<Vec<String>>,
    pub truncated: bool,
    pub summary: String,
}

fn extract_tables_recursive<'a>(
    iter: impl Iterator<Item = (&'a str, &'a Item)>,
    prefix: &str,
) -> Vec<String> {
    let mut tables = Vec::new();
    for (key, item) in iter {
        let full_name = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", prefix, key)
        };
        match item {
            Item::Table(table) => {
                tables.push(full_name.clone());
                tables.extend(extract_tables_recursive(table.iter(), &full_name));
            }
            Item::ArrayOfTables(aot) => {
                tables.push(full_name.clone());
                if let Some(first) = aot.iter().next() {
                    tables.extend(extract_tables_recursive(first.iter(), &full_name));
                }
            }
            _ => {}
        }
    }
    tables
}

pub fn validate_toml(text: &str) -> Result<ValidateTomlResult, String> {
    if text.len() > MAX_INPUT_LENGTH {
        return Err(format!(
            "Input length {} exceeds MAX_INPUT_LENGTH {}",
            text.len(),
            MAX_INPUT_LENGTH
        ));
    }

    match text.parse::<DocumentMut>() {
        Ok(doc) => {
            let top_level_keys: Vec<String> = doc.iter().map(|(k, _)| k.to_string()).collect();
            let tables = extract_tables_recursive(doc.iter(), "");

            Ok(ValidateTomlResult {
                valid: true,
                error: None,
                line: None,
                column: None,
                position: None,
                toml_type: Some("document".to_string()),
                top_level_keys: Some(top_level_keys),
                tables: Some(tables),
            })
        }
        Err(e) => {
            let position = e.span().map(|s| s.start as i32);
            let line_col = position.and_then(|pos| {
                if pos >= 0 {
                    Some(byte_offset_to_line_col(text, pos as usize))
                } else {
                    None
                }
            });
            // Build a single-line error message that matches Python's tomllib
            // format. tomllib returns "<description> (at <position>)" where
            // position is "end of document" or "line N column M". toml_edit's
            // e.message() is sometimes empty (e.g. "key =" with missing
            // value), and the message can span multiple lines. We extract
            // the first non-empty line and synthesize a Python-compatible
            // message in tomllib's format.
            let raw = e.message();
            let first_line = raw
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("Invalid value");
            let span = e.span();
            let at_end_of_document = span
                .as_ref()
                .map(|s| s.start >= text.len())
                .unwrap_or(false);
            let err_str = if at_end_of_document {
                format!("{} (at end of document)", first_line)
            } else {
                match (line_col, position) {
                    (Some((l, c)), _) => format!("{} (at line {}, column {})", first_line, l, c),
                    (None, Some(p)) => format!("{} (at line ?, column {})", first_line, p),
                    (None, None) => first_line.to_string(),
                }
            };
            Ok(ValidateTomlResult {
                valid: false,
                error: Some(err_str),
                line: line_col.map(|(l, _)| l),
                column: line_col.map(|(_, c)| c),
                position,
                toml_type: None,
                top_level_keys: None,
                tables: None,
            })
        }
    }
}

pub fn toml_shape(text: &str, max_tables: usize) -> Result<TomlShapeResult, String> {
    if text.len() > MAX_INPUT_LENGTH {
        return Err(format!(
            "Input length {} exceeds MAX_INPUT_LENGTH {}",
            text.len(),
            MAX_INPUT_LENGTH
        ));
    }

    match text.parse::<DocumentMut>() {
        Ok(doc) => {
            let top_level_keys: Vec<String> = doc.iter().map(|(k, _)| k.to_string()).collect();
            let all_tables = extract_tables_recursive(doc.iter(), "");

            let total_table_count = all_tables.len();
            let truncated = total_table_count > max_tables;
            let tables = if truncated {
                all_tables.into_iter().take(max_tables).collect()
            } else {
                all_tables
            };

            let key_count = top_level_keys.len();
            Ok(TomlShapeResult {
                valid: true,
                top_level_keys: Some(top_level_keys),
                tables: Some(tables),
                truncated,
                summary: format!(
                    "Valid TOML with {} top-level keys and {} tables",
                    key_count, total_table_count
                ),
            })
        }
        Err(e) => Ok(TomlShapeResult {
            valid: false,
            top_level_keys: None,
            tables: None,
            truncated: false,
            summary: format!("Error: {}", e),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_toml_valid() {
        let result = validate_toml("[package]\nname = \"test\"").unwrap();
        assert!(result.valid);
        assert!(result.error.is_none());
        assert_eq!(result.toml_type, Some("document".to_string()));
        assert!(result.top_level_keys.is_some());
        assert!(result.tables.is_some());
    }

    #[test]
    fn test_validate_toml_top_level_keys() {
        let result = validate_toml("key = \"value\"\n[table]\nfoo = 1").unwrap();
        assert!(result.valid);
        let keys = result.top_level_keys.unwrap();
        assert!(keys.contains(&"key".to_string()));
    }

    #[test]
    fn test_validate_toml_tables() {
        let result = validate_toml("[package]\nname = \"test\"\n[dependencies]").unwrap();
        assert!(result.valid);
        let tables = result.tables.unwrap();
        assert!(tables.contains(&"package".to_string()));
        assert!(tables.contains(&"dependencies".to_string()));
    }

    #[test]
    fn test_validate_toml_nested_tables() {
        let result = validate_toml("[package]\nname = \"test\"\n[dependencies.dev]").unwrap();
        assert!(result.valid);
        let tables = result.tables.unwrap();
        assert!(tables.contains(&"package".to_string()));
        assert!(tables.contains(&"dependencies".to_string()));
        assert!(tables.contains(&"dependencies.dev".to_string()));
    }

    #[test]
    fn test_validate_toml_invalid() {
        let result = validate_toml("[invalid\n.toml").unwrap();
        assert!(!result.valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_validate_toml_error_position() {
        let result = validate_toml("key = value").unwrap();
        assert!(!result.valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_validate_toml_error_has_line_col() {
        let result = validate_toml("[invalid\n.toml").unwrap();
        assert!(!result.valid);
        assert!(result.line.is_some(), "Error should have line info");
        assert!(result.column.is_some(), "Error should have column info");
    }

    #[test]
    fn test_toml_shape_valid() {
        let result = toml_shape("[package]\nname = \"test\"", 100).unwrap();
        assert!(result.valid);
        assert!(result.top_level_keys.is_some());
        assert!(result.tables.is_some());
        assert!(!result.truncated);
    }

    #[test]
    fn test_toml_shape_truncated() {
        let toml_text = "[table1]\na=1\n[table2]\nb=2\n[table3]\nc=3";
        let result = toml_shape(toml_text, 2).unwrap();
        assert!(result.valid);
        assert!(result.truncated);
        let tables = result.tables.unwrap();
        assert_eq!(tables.len(), 2);
    }

    #[test]
    fn test_toml_shape_invalid() {
        let result = toml_shape("invalid toml", 100).unwrap();
        assert!(!result.valid);
        assert!(result.top_level_keys.is_none());
        assert!(result.summary.contains("Error:"));
    }

    #[test]
    fn test_toml_shape_empty() {
        let result = toml_shape("", 100).unwrap();
        assert!(result.valid);
        assert!(result.top_level_keys.unwrap().is_empty());
    }

    #[test]
    fn test_toml_tables_excludes_scalar_keys() {
        let input =
            "[package]\nname = \"test\"\nversion = \"1.0\"\n\n[dependencies]\nserde = \"1\"";
        let result = validate_toml(input).unwrap();
        let tables = result.tables.unwrap();
        assert!(
            tables.contains(&"package".to_string()),
            "package should be a table"
        );
        assert!(
            tables.contains(&"dependencies".to_string()),
            "dependencies should be a table"
        );
        assert!(
            !tables.contains(&"package.name".to_string()),
            "package.name is a scalar, not a table"
        );
        assert!(
            !tables.contains(&"package.version".to_string()),
            "package.version is a scalar"
        );
        assert!(
            !tables.contains(&"dependencies.serde".to_string()),
            "dependencies.serde is a scalar"
        );
    }

    #[test]
    fn test_toml_shape_scalar_excluded() {
        let input = "[package]\nname = \"test\"\nversion = \"1.0\"";
        let result = toml_shape(input, 100).unwrap();
        let tables = result.tables.unwrap();
        assert_eq!(tables, vec!["package"]);
    }

    #[test]
    fn test_toml_arrays_of_tables() {
        let input = "[[products]]\nname = \"hammer\"\n[[products]]\nname = \"nail\"";
        let result = validate_toml(input).unwrap();
        let tables = result.tables.unwrap();
        assert!(
            tables.contains(&"products".to_string()),
            "array of tables should appear once"
        );
        assert_eq!(
            tables.iter().filter(|t| *t == "products").count(),
            1,
            "array of tables should appear exactly once"
        );
    }

    #[test]
    fn test_toml_dotted_table_names() {
        let input = "[a.b.c]\nkey = \"value\"";
        let result = validate_toml(input).unwrap();
        let tables = result.tables.unwrap();
        assert!(tables.contains(&"a".to_string()));
        assert!(tables.contains(&"a.b".to_string()));
        assert!(tables.contains(&"a.b.c".to_string()));
    }

    #[test]
    fn test_toml_unicode_column_after_multibyte() {
        let input = "key = \"\u{00e9}\"\nbad = = =";
        let result = validate_toml(input).unwrap();
        assert!(!result.valid);
        if let (Some(line), Some(col)) = (result.line, result.column) {
            assert_eq!(line, 2, "Error should be on line 2");
            assert!(col >= 1, "Column should be >= 1, got {}", col);
        }
    }

    #[test]
    fn test_toml_shape_summary_uses_total_table_count() {
        let input = "[a]\nx=1\n[b]\ny=2\n[c]\nz=3";
        let result = toml_shape(input, 1).unwrap();
        assert!(result.truncated);
        assert_eq!(result.tables.as_ref().unwrap().len(), 1);
        assert!(
            result.summary.contains("3 tables"),
            "Summary should report total count, not truncated: {}",
            result.summary
        );
    }

    #[test]
    fn test_toml_inline_table_not_listed() {
        let input = "config = {key = \"value\"}";
        let result = validate_toml(input).unwrap();
        let tables = result.tables.unwrap();
        assert!(
            tables.is_empty(),
            "inline table should not be listed as a table"
        );
    }
}
