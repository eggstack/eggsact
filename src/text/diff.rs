#[allow(clippy::needless_range_loop)]
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    levenshtein_distance_with_limit(a, b, 10000)
}

pub fn levenshtein_distance_with_limit(a: &str, b: &str, max_len: usize) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len > max_len || b_len > max_len {
        return std::cmp::max(a_len, b_len);
    }

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(a_len + 1) {
        row[0] = i;
    }
    for (j, cell) in matrix[0].iter_mut().enumerate().take(b_len + 1) {
        *cell = j;
    }

    for (i, ac) in a_chars.iter().enumerate() {
        for (j, bc) in b_chars.iter().enumerate() {
            let cost = if ac == bc { 0 } else { 1 };
            matrix[i + 1][j + 1] = std::cmp::min(
                std::cmp::min(matrix[i][j + 1] + 1, matrix[i + 1][j] + 1),
                matrix[i][j] + cost,
            );
        }
    }

    matrix[a_len][b_len]
}

#[derive(Debug, serde::Serialize)]
pub struct DiffSpan {
    pub a_start: usize,
    pub a_end: usize,
    pub b_start: usize,
    pub b_end: usize,
    pub kind: String,
    pub a_text: String,
    pub b_text: String,
}

fn char_slice(s: &str, chars: &[char], start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }
    let byte_start = chars[..start].iter().map(|c| c.len_utf8()).sum::<usize>();
    let byte_end = chars[..end].iter().map(|c| c.len_utf8()).sum::<usize>();
    s[byte_start..byte_end].to_string()
}

pub fn diff_spans(a: &str, b: &str, max_diffs: usize) -> Vec<DiffSpan> {
    if a.is_empty() && b.is_empty() {
        return vec![];
    }

    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    // LCS dynamic programming with backtracking to find matching blocks.
    // This mirrors Python's difflib.SequenceMatcher which uses an LCS-based
    // approach to find optimal matching blocks and produce edit opcodes.
    let mut dp = vec![vec![0u32; b_len + 1]; a_len + 1];
    for i in 1..=a_len {
        for j in 1..=b_len {
            if a_chars[i - 1] == b_chars[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j - 1].max(dp[i - 1][j]).max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to collect matching positions in reverse order.
    let mut match_positions_rev: Vec<(usize, usize)> = vec![];
    let mut i = a_len;
    let mut j = b_len;
    while i > 0 && j > 0 {
        if a_chars[i - 1] == b_chars[j - 1] {
            match_positions_rev.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    // Reverse to forward order.
    match_positions_rev.reverse();

    // Merge contiguous matches into blocks: (a_start, a_end, b_start, b_end).
    let mut blocks: Vec<(usize, usize, usize, usize)> = vec![];
    for (ai, bj) in &match_positions_rev {
        if let Some(last) = blocks.last_mut() {
            if last.1 == *ai && last.3 == *bj {
                last.1 = ai + 1;
                last.3 = bj + 1;
                continue;
            }
        }
        blocks.push((*ai, ai + 1, *bj, bj + 1));
    }

    // Convert matching blocks into diff spans (skip equal regions).
    let mut spans: Vec<DiffSpan> = vec![];
    let mut prev_a_end = 0;
    let mut prev_b_end = 0;

    for (a_start, a_end, b_start, b_end) in &blocks {
        if prev_a_end < *a_start || prev_b_end < *b_start {
            let kind = if prev_a_end < *a_start && prev_b_end < *b_start {
                "replace"
            } else if prev_b_end < *b_start {
                "insert"
            } else {
                "delete"
            };
            spans.push(DiffSpan {
                a_start: prev_a_end,
                a_end: *a_start,
                b_start: prev_b_end,
                b_end: *b_start,
                kind: kind.to_string(),
                a_text: char_slice(a, &a_chars, prev_a_end, *a_start),
                b_text: char_slice(b, &b_chars, prev_b_end, *b_start),
            });
            if spans.len() >= max_diffs {
                break;
            }
        }
        prev_a_end = *a_end;
        prev_b_end = *b_end;
    }

    // Emit any trailing non-equal region after the last match.
    if spans.len() < max_diffs && (prev_a_end < a_len || prev_b_end < b_len) {
        let kind = if prev_a_end < a_len && prev_b_end < b_len {
            "replace"
        } else if prev_b_end < b_len {
            "insert"
        } else {
            "delete"
        };
        spans.push(DiffSpan {
            a_start: prev_a_end,
            a_end: a_len,
            b_start: prev_b_end,
            b_end: b_len,
            kind: kind.to_string(),
            a_text: char_slice(a, &a_chars, prev_a_end, a_len),
            b_text: char_slice(b, &b_chars, prev_b_end, b_len),
        });
    }

    spans
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FirstDiff {
    pub a_index: usize,
    pub b_index: usize,
    pub a_char: String,
    pub b_char: String,
    pub a_codepoint: String,
    pub b_codepoint: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommonPrefixSuffix {
    pub common_prefix_len: usize,
    pub common_suffix_len: usize,
}

pub fn first_diff(a: &str, b: &str) -> Option<FirstDiff> {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let min_len = a_chars.len().min(b_chars.len());

    for i in 0..min_len {
        if a_chars[i] != b_chars[i] {
            return Some(FirstDiff {
                a_index: i,
                b_index: i,
                a_char: a_chars[i].to_string(),
                b_char: b_chars[i].to_string(),
                a_codepoint: format!("U+{:04X}", a_chars[i] as u32),
                b_codepoint: format!("U+{:04X}", b_chars[i] as u32),
            });
        }
    }

    if a_chars.len() != b_chars.len() {
        let idx = min_len;
        let a_char = if a_chars.len() > min_len {
            a_chars[min_len].to_string()
        } else {
            String::new()
        };
        let b_char = if b_chars.len() > min_len {
            b_chars[min_len].to_string()
        } else {
            String::new()
        };
        let a_codepoint = if a_chars.len() > min_len {
            format!("U+{:04X}", a_chars[min_len] as u32)
        } else {
            String::new()
        };
        let b_codepoint = if b_chars.len() > min_len {
            format!("U+{:04X}", b_chars[min_len] as u32)
        } else {
            String::new()
        };
        return Some(FirstDiff {
            a_index: idx,
            b_index: idx,
            a_char,
            b_char,
            a_codepoint,
            b_codepoint,
        });
    }

    None
}

pub fn common_prefix_suffix(a: &str, b: &str) -> CommonPrefixSuffix {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let min_len = a_chars.len().min(b_chars.len());

    let mut prefix_len = 0;
    while prefix_len < min_len && a_chars[prefix_len] == b_chars[prefix_len] {
        prefix_len += 1;
    }

    let mut suffix_len = 0;
    while suffix_len < min_len - prefix_len
        && a_chars[a_chars.len() - 1 - suffix_len] == b_chars[b_chars.len() - 1 - suffix_len]
    {
        suffix_len += 1;
    }

    CommonPrefixSuffix {
        common_prefix_len: prefix_len,
        common_suffix_len: suffix_len,
    }
}
