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
        return ToolResponse::error(
            "input_too_large",
            &format!("List length exceeds MAX_LIST_ITEMS {}", MAX_LIST_ITEMS),
            Some(vec![format!("Maximum {} items per list", MAX_LIST_ITEMS)]),
            Some("list_compare"),
        );
    }

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
        return ToolResponse::error(
            "invalid_arguments",
            "All list elements must be strings",
            Some(errors.into_iter().take(10).collect()),
            Some("list_compare"),
        );
    }

    let max_total_chars = MAX_TEXT_LENGTH * 2;
    if total_chars > max_total_chars {
        return ToolResponse::error(
            "input_too_large",
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
    let _ignore_order = args
        .get("ignore_order")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| mode != "ordered");
    let treat_as_multiset = args
        .get("treat_as_multiset")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| mode == "multiset");

    let valid_modes = ["ordered", "set", "multiset"];
    if !valid_modes.contains(&mode) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported mode: {}", mode),
            Some(vec![format!("Use one of: {}", valid_modes.join(", "))]),
            Some("list_compare"),
        );
    }

    let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalization) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("list_compare"),
        );
    }

    if near_match_threshold < 0.0 {
        return ToolResponse::error(
            "invalid_arguments",
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
    let _ignore_order_val = if let Some(v) = args.get("ignore_order").and_then(|v| v.as_bool()) {
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

    let only_a_orig: Vec<Value> = if treat_as_multiset_val {
        a.iter()
            .enumerate()
            .filter(|(i, _)| {
                let t = &a_transformed[*i];
                a_counts.get(t).copied().unwrap_or(0) > b_counts.get(t).copied().unwrap_or(0)
            })
            .map(|(_, v)| v.clone())
            .collect()
    } else {
        a.iter()
            .filter(|v| !b_set.contains(&transform(v)))
            .cloned()
            .collect()
    };
    let only_b_orig: Vec<Value> = if treat_as_multiset_val {
        b.iter()
            .enumerate()
            .filter(|(i, _)| {
                let t = &b_transformed[*i];
                b_counts.get(t).copied().unwrap_or(0) > a_counts.get(t).copied().unwrap_or(0)
            })
            .map(|(_, v)| v.clone())
            .collect()
    } else {
        b.iter()
            .filter(|v| !a_set.contains(&transform(v)))
            .cloned()
            .collect()
    };

    let intersection: Vec<Value> = if treat_as_multiset_val {
        let mut result = Vec::new();
        let mut used_a: Vec<bool> = vec![false; a.len()];
        let mut used_b: Vec<bool> = vec![false; b.len()];
        for (ai, at) in a_transformed.iter().enumerate() {
            for (bi, bt) in b_transformed.iter().enumerate() {
                if !used_a[ai] && !used_b[bi] && at == bt {
                    used_a[ai] = true;
                    used_b[bi] = true;
                    result.push(a[ai].clone());
                    break;
                }
            }
        }
        result
    } else {
        let common: std::collections::HashSet<&String> = a_set.intersection(&b_set).collect();
        let mut result: Vec<Value> = Vec::new();
        for item in a.iter() {
            let t = transform(item);
            if common.contains(&t) {
                result.push(item.clone());
            }
        }
        result
    };

    let mut near_matches: Vec<serde_json::Value> = Vec::new();
    if include_near_matches && !only_a_orig.is_empty() && !only_b_orig.is_empty() {
        for (ai, av) in only_a_orig.iter().enumerate() {
            let a_str = av.as_str().unwrap_or("");
            for (bi, bv) in only_b_orig.iter().enumerate() {
                let b_str = bv.as_str().unwrap_or("");
                let dist = levenshtein_distance(a_str, b_str);
                let max_len = a_str.chars().count().max(b_str.chars().count());
                if max_len > 0 && (dist as f64) <= near_match_threshold {
                    near_matches.push(serde_json::json!({
                        "left_index": ai,
                        "right_index": bi,
                        "left": a_str,
                        "right": b_str,
                        "distance": dist,
                    }));
                }
            }
        }
    }

    let summary = if only_a_orig.is_empty() && only_b_orig.is_empty() {
        "Lists are equivalent".to_string()
    } else {
        format!(
            "{} items only in A, {} items only in B, {} in intersection",
            only_a_orig.len(),
            only_b_orig.len(),
            intersection.len()
        )
    };

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "mode": mode,
            "only_in_a": only_a_orig,
            "only_in_b": only_b_orig,
            "intersection": intersection,
            "summary": summary,
        }),
        Some("list_compare"),
    )
    .with_tool("list_compare");

    if include_near_matches && !near_matches.is_empty() {
        resp = resp.with_findings(near_matches);
    }

    resp
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev = vec![0usize; b_len + 1];
    let mut curr = vec![0usize; b_len + 1];

    for (j, item) in prev.iter_mut().enumerate() {
        *item = j;
    }

    for i in 1..=a_len {
        curr[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}

pub fn list_dedupe(args: &Value) -> ToolResponse {
    let items = match require_array_arg(args, "items", "list_dedupe") {
        Ok(items) => items,
        Err(response) => return *response,
    };
    if items.len() > MAX_LIST_ITEMS {
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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
