use crate::mcp::machine_codes;
use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

pub fn list_compare(args: &Value) -> ToolResponse {
    let (a, b) = match require_list_compare_args(args) {
        Ok(values) => values,
        Err(response) => return *response,
    };

    if a.len() > MAX_LIST_ITEMS || b.len() > MAX_LIST_ITEMS {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("List length exceeds MAX_LIST_ITEMS {}", MAX_LIST_ITEMS),
            Some(vec![format!("Maximum {} items per list", MAX_LIST_ITEMS)]),
            Some("list_compare"),
        );
    }

    // Validate all elements are strings
    let mut total_chars = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for (i, item) in a.iter().enumerate() {
        if !item.is_string() {
            errors.push(format!("[{}] is {}, not string", i, json_type_name(item)));
        } else {
            total_chars += item.as_str().unwrap_or("").chars().count();
        }
    }
    for (i, item) in b.iter().enumerate() {
        if !item.is_string() {
            errors.push(format!("[{}] is {}, not string", i, json_type_name(item)));
        } else {
            total_chars += item.as_str().unwrap_or("").chars().count();
        }
    }
    if !errors.is_empty() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "All list elements must be strings",
            Some(errors.into_iter().take(10).collect()),
            Some("list_compare"),
        );
    }

    let max_total_chars = MAX_TEXT_LENGTH * 2;
    if total_chars > max_total_chars {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Total string length {} exceeds maximum", total_chars),
            Some(vec![format!(
                "Maximum combined string length is {} characters",
                max_total_chars
            )]),
            Some("list_compare"),
        );
    }

    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("set");
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("NFC");
    let trim = args.get("trim").and_then(|v| v.as_bool()).unwrap_or(false);
    let include_near_matches = args
        .get("include_near_matches")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let near_match_threshold = args
        .get("near_match_threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(2.0);
    // Match Python: ignore_order defaults to mode != "ordered" when not provided
    let _ignore_order = args
        .get("ignore_order")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| mode != "ordered");
    // Match Python: treat_as_multiset defaults to mode == "multiset" when not provided
    let treat_as_multiset = args
        .get("treat_as_multiset")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| mode == "multiset");

    let valid_modes = ["ordered", "set", "multiset"];
    if !valid_modes.contains(&mode) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported mode: {}", mode),
            Some(vec![format!("Use one of: {}", valid_modes.join(", "))]),
            Some("list_compare"),
        );
    }

    let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalization) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("list_compare"),
        );
    }

    if near_match_threshold < 0.0 {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "near_match_threshold must be non-negative, got {}",
                near_match_threshold
            ),
            Some(vec!["Set near_match_threshold to 0 or higher".to_string()]),
            Some("list_compare"),
        );
    }

    let treat_as_multiset_val = if mode == "multiset" {
        true
    } else {
        treat_as_multiset
    };
    let ignore_order_val = if let Some(v) = args.get("ignore_order").and_then(|v| v.as_bool()) {
        v
    } else {
        mode != "ordered"
    };

    let transform = |v: &Value| -> String {
        let mut result = match v.as_str() {
            Some(s) => s.to_string(),
            None => v.to_string(),
        };
        if trim {
            result = result.trim().to_string();
        }
        if normalization != "raw" {
            result = match normalization {
                "NFC" => result.nfc().to_string(),
                "NFD" => result.nfd().to_string(),
                "NFKC" => result.nfkc().to_string(),
                "NFKD" => result.nfkd().to_string(),
                _ => result,
            };
        }
        if casefold {
            result = unicode_casefold(&result);
        }
        result
    };

    let a_transformed: Vec<String> = a.iter().map(&transform).collect();
    let b_transformed: Vec<String> = b.iter().map(&transform).collect();

    use std::collections::HashMap;
    let mut a_counts: HashMap<String, usize> = HashMap::new();
    let mut b_counts: HashMap<String, usize> = HashMap::new();
    for x in &a_transformed {
        *a_counts.entry(x.clone()).or_insert(0) += 1;
    }
    for x in &b_transformed {
        *b_counts.entry(x.clone()).or_insert(0) += 1;
    }

    let a_set: std::collections::HashSet<String> = a_transformed.iter().cloned().collect();
    let b_set: std::collections::HashSet<String> = b_transformed.iter().cloned().collect();

    // In set mode, only_in_a/only_in_b use set membership (items not in other set at all)
    // In multiset mode, use count comparison (items where count_a > count_b)
    let only_a_orig: Vec<Value> = if treat_as_multiset_val {
        // multiset: use count comparison - items where a count > b count
        a.iter()
            .enumerate()
            .filter(|(i, _)| {
                let t = &a_transformed[*i];
                a_counts.get(t).copied().unwrap_or(0) > b_counts.get(t).copied().unwrap_or(0)
            })
            .map(|(_, v)| v.clone())
            .collect()
    } else {
        // set: use set membership - items not present in b at all
        a.iter()
            .filter(|v| !b_set.contains(&transform(v)))
            .cloned()
            .collect()
    };
    let only_b_orig: Vec<Value> = if treat_as_multiset_val {
        // multiset: use count comparison - items where b count > a count
        b.iter()
            .enumerate()
            .filter(|(i, _)| {
                let t = &b_transformed[*i];
                b_counts.get(t).copied().unwrap_or(0) > a_counts.get(t).copied().unwrap_or(0)
            })
            .map(|(_, v)| v.clone())
            .collect()
    } else {
        // set: use set membership - items not present in a at all
        b.iter()
            .filter(|v| !a_set.contains(&transform(v)))
            .cloned()
            .collect()
    };

    let duplicates_a: Vec<String>;
    let duplicates_b: Vec<String>;
    {
        duplicates_a = a_counts
            .iter()
            .filter(|(_, c)| **c > 1)
            .map(|(k, _)| k.clone())
            .collect();
        duplicates_b = b_counts
            .iter()
            .filter(|(_, c)| **c > 1)
            .map(|(k, _)| k.clone())
            .collect();
    }

    let mut near_matches: Vec<serde_json::Value> = Vec::new();
    if include_near_matches && near_match_threshold > 0.0 {
        let threshold_int = near_match_threshold.round() as usize;
        if threshold_int > 0 {
            let mut seen_pairs: std::collections::HashSet<(String, String)> =
                std::collections::HashSet::new();
            for (i, a_item) in a.iter().enumerate() {
                let a_t = &a_transformed[i];
                for (j, b_item) in b.iter().enumerate() {
                    let b_t = &b_transformed[j];
                    if a_t == b_t {
                        continue;
                    }
                    let dist = crate::text::levenshtein_distance(a_t, b_t);
                    if dist > 0 && dist <= threshold_int {
                        let a_str = a_item.as_str().unwrap_or("");
                        let b_str = b_item.as_str().unwrap_or("");
                        let pair = if a_str <= b_str {
                            (a_str.to_string(), b_str.to_string())
                        } else {
                            (b_str.to_string(), a_str.to_string())
                        };
                        if !seen_pairs.contains(&pair) {
                            seen_pairs.insert(pair);
                            near_matches.push(serde_json::json!({
                                "a": a_item,
                                "b": b_item,
                                "distance": dist,
                                "classification": "fuzzy"
                            }));
                        }
                        break;
                    }
                }
            }
        }
    }

    let same_ordered = ignore_order_val || (a_transformed == b_transformed);

    let same_unordered = if treat_as_multiset_val {
        a_counts == b_counts
    } else {
        a_set == b_set
    };

    let equal = match mode {
        "ordered" => same_ordered,
        "set" => same_unordered && only_a_orig.is_empty() && only_b_orig.is_empty(),
        _ => same_unordered,
    };

    let only_in_a: Vec<Value> = only_a_orig.to_vec();
    let only_in_b: Vec<Value> = only_b_orig.to_vec();

    if mode == "ordered" {
        let mut aligned: Vec<serde_json::Value> = Vec::new();
        let max_len = std::cmp::max(a.len(), b.len());
        let mut first_diff_index: Option<usize> = None;

        for i in 0..max_len {
            let a_item = if i < a.len() {
                Some(a[i].clone())
            } else {
                None
            };
            let b_item = if i < b.len() {
                Some(b[i].clone())
            } else {
                None
            };

            let op = match (&a_item, &b_item) {
                (Some(_), None) => "delete",
                (None, Some(_)) => "insert",
                (Some(a_val), Some(b_val)) if a_transformed.get(i) == b_transformed.get(i) => {
                    "equal"
                }
                _ => "replace",
            };

            if first_diff_index.is_none() && op != "equal" {
                first_diff_index = Some(i);
            }

            let mut entry = serde_json::json!({"op": op});
            if let Some(ref v) = a_item {
                entry["a"] = v.clone();
                entry["a_index"] = Value::from(i);
            }
            if let Some(ref v) = b_item {
                entry["b"] = v.clone();
                entry["b_index"] = Value::from(i);
            }
            aligned.push(entry);
        }

        let equal_prefix_length = first_diff_index.unwrap_or(a.len());

        ToolResponse::success(
            serde_json::json!({
                "equal": equal,
                "first_diff_index": first_diff_index,
                "equal_prefix_length": equal_prefix_length,
                "aligned": aligned,
                "only_in_a": only_in_a,
                "only_in_b": only_in_b,
                "missing_in_a": only_in_b,
                "missing_in_b": only_in_a,
                "duplicates_in_a": duplicates_a,
                "duplicates_in_b": duplicates_b,
                "near_matches": near_matches,
            }),
            Some("list_compare"),
        )
        .with_tool("list_compare")
    } else if mode == "set" {
        ToolResponse::success(
            serde_json::json!({
                "equal": equal,
                "only_in_a": only_in_a,
                "only_in_b": only_in_b,
                "missing_in_a": only_in_b,
                "missing_in_b": only_in_a,
                "duplicates_in_a": duplicates_a,
            "duplicates_in_b": duplicates_b,
            "near_matches": near_matches,
            }),
            Some("list_compare"),
        )
        .with_tool("list_compare")
    } else {
        let mut count_deltas: serde_json::Map<String, Value> = serde_json::Map::new();
        let all_keys: std::collections::HashSet<String> =
            a_counts.keys().chain(b_counts.keys()).cloned().collect();
        for k in all_keys {
            let delta =
                *a_counts.get(&k).unwrap_or(&0) as i64 - *b_counts.get(&k).unwrap_or(&0) as i64;
            if delta != 0 {
                count_deltas.insert(k, Value::Number(delta.into()));
            }
        }

        ToolResponse::success(
            serde_json::json!({
                "equal": equal,
                "count_deltas": Value::Object(count_deltas),
                "missing_in_a": only_in_b,
                "missing_in_b": only_in_a,
                "duplicates_in_a": duplicates_a,
                "duplicates_in_b": duplicates_b,
                "only_in_a": only_in_a,
                "only_in_b": only_in_b,
                "near_matches": near_matches,
            }),
            Some("list_compare"),
        )
        .with_tool("list_compare")
    }
}

pub fn list_dedupe(args: &Value) -> ToolResponse {
    let items = match require_array_arg(args, "items", "list_dedupe") {
        Ok(items) => items,
        Err(response) => return *response,
    };
    if items.len() > MAX_LIST_ITEMS {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("items length {} exceeds {}", items.len(), MAX_LIST_ITEMS),
            None,
            Some("list_dedupe"),
        );
    }
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("NFC");
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let stable = args.get("stable").and_then(|v| v.as_bool()).unwrap_or(true);

    let non_str_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_string())
        .map(|(i, _)| i)
        .collect();
    if !non_str_indices.is_empty() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "All items elements must be strings",
            Some(vec![format!(
                "Non-string items at indices: {:?}",
                &non_str_indices[..5.min(non_str_indices.len())]
            )]),
            Some("list_dedupe"),
        );
    }

    let oversized_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.as_str()
                .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
        })
        .map(|(i, _)| i)
        .collect();
    if !oversized_indices.is_empty() {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("items exceed max length {}", MAX_TEXT_LENGTH),
            Some(vec![format!(
                "Oversized items at indices: {:?}",
                &oversized_indices[..5.min(oversized_indices.len())]
            )]),
            Some("list_dedupe"),
        );
    }

    let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalization) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("list_dedupe"),
        );
    }

    let original_count = items.len();

    let normalize_item = |item: &Value| -> String {
        let s = match item.as_str() {
            Some(st) => st.to_string(),
            None => item.to_string(),
        };
        let mut compare_val = if casefold { unicode_casefold(&s) } else { s };
        if normalization != "raw" {
            compare_val = match normalization {
                "NFD" => compare_val.nfd().collect::<String>(),
                "NFKC" => compare_val.nfkc().collect::<String>(),
                "NFKD" => compare_val.nfkd().collect::<String>(),
                _ => compare_val.nfc().collect::<String>(),
            };
        }
        compare_val
    };

    let result: Vec<serde_json::Value> = if stable {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out: Vec<serde_json::Value> = Vec::new();
        for item in items {
            let key = normalize_item(item);
            if seen.insert(key) {
                out.push(item.clone());
            }
        }
        out
    } else {
        let mut unique_map: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::new();
        for item in items {
            let key = normalize_item(item);
            unique_map.entry(key).or_insert_with(|| item.clone());
        }
        unique_map.into_values().collect()
    };

    let deduped_count = result.len();
    ToolResponse::success(
        serde_json::json!({
            "items": result,
            "original_count": original_count,
            "deduped_count": deduped_count,
            "duplicates_removed": original_count - deduped_count,
        }),
        Some("list_dedupe"),
    )
    .with_tool("list_dedupe")
}

pub fn list_sort(args: &Value) -> ToolResponse {
    let items = match require_array_arg(args, "items", "list_sort") {
        Ok(items) => items,
        Err(response) => return *response,
    };
    if items.len() > MAX_LIST_ITEMS {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("items length {} exceeds {}", items.len(), MAX_LIST_ITEMS),
            None,
            Some("list_sort"),
        );
    }
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("NFC");
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let reverse = args
        .get("reverse")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let stable = args.get("stable").and_then(|v| v.as_bool()).unwrap_or(true);

    let non_str_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_string())
        .map(|(i, _)| i)
        .collect();
    if !non_str_indices.is_empty() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "All items elements must be strings",
            Some(vec![format!(
                "Non-string items at indices: {:?}",
                &non_str_indices[..5.min(non_str_indices.len())]
            )]),
            Some("list_sort"),
        );
    }

    let oversized_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.as_str()
                .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
        })
        .map(|(i, _)| i)
        .collect();
    if !oversized_indices.is_empty() {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("items exceed max length {}", MAX_TEXT_LENGTH),
            Some(vec![format!(
                "Oversized items at indices: {:?}",
                &oversized_indices[..5.min(oversized_indices.len())]
            )]),
            Some("list_sort"),
        );
    }

    let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalization) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("list_sort"),
        );
    }

    let mut paired: Vec<(String, serde_json::Value)> = Vec::new();
    for item in items {
        let s = match item.as_str() {
            Some(st) => st.to_string(),
            None => item.to_string(),
        };
        let mut key = if casefold { unicode_casefold(&s) } else { s };
        if normalization != "raw" {
            key = match normalization {
                "NFD" => key.nfd().collect::<String>(),
                "NFKC" => key.nfkc().collect::<String>(),
                "NFKD" => key.nfkd().collect::<String>(),
                _ => key.nfc().collect::<String>(),
            };
        }
        paired.push((key, item.clone()));
    }

    if stable {
        paired.sort_by(|a, b| a.0.cmp(&b.0));
    } else {
        paired.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    }
    if reverse {
        paired.reverse();
    }

    let original_count = items.len();
    let sorted_items: Vec<serde_json::Value> = paired.into_iter().map(|(_, v)| v).collect();
    let sorted_count = sorted_items.len();

    ToolResponse::success(
        serde_json::json!({
            "items": sorted_items,
            "original_count": original_count,
            "sorted_count": sorted_count,
        }),
        Some("list_sort"),
    )
    .with_tool("list_sort")
}
