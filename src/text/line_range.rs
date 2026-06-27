use serde::{Deserialize, Serialize};

const MAX_TEXT_LENGTH: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineExtractFinding {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineExtractLine {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRangeExtractResult {
    pub line_count_total: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub valid_range: bool,
    pub text: String,
    pub lines: Vec<LineExtractLine>,
    pub byte_start: usize,
    pub byte_end: usize,
    pub char_start: usize,
    pub char_end: usize,
    pub newline_style: String,
    pub ends_with_newline: bool,
    pub fingerprint: String,
    pub findings: Vec<LineExtractFinding>,
}

fn detect_newline_style(s: &str) -> &'static str {
    let crlf_count = s.matches("\r\n").count();
    let lf_only = s.matches('\n').count() - crlf_count;
    let cr_only = s.matches('\r').count() - crlf_count;

    let has_crlf = crlf_count > 0;
    let has_lf = lf_only > 0;
    let has_cr = cr_only > 0;

    if has_crlf && (has_lf || has_cr) {
        return "mixed";
    }
    if has_crlf {
        return "CRLF";
    }
    if has_lf {
        return "LF";
    }
    if has_cr {
        return "CR";
    }
    "none"
}

fn fingerprint(text: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let result = hasher.finalize();
    let hex = format!("{:x}", result);
    hex[..16].to_string()
}

#[derive(Debug, Clone, Copy)]
struct LineSegment<'a> {
    text: &'a str,
    byte_start: usize,
    byte_end: usize,
}

fn line_number_to_one_based(line: usize, line_base: usize) -> Result<usize, String> {
    match line_base {
        0 => line
            .checked_add(1)
            .ok_or_else(|| "line number is too large".to_string()),
        1 => Ok(line),
        _ => Err(format!("line_base must be 0 or 1, got {}", line_base)),
    }
}

fn split_line_segments(text: &str) -> Vec<LineSegment<'_>> {
    if text.is_empty() {
        return vec![LineSegment {
            text: "",
            byte_start: 0,
            byte_end: 0,
        }];
    }

    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut iter = text.char_indices().peekable();

    while let Some((idx, ch)) = iter.next() {
        match ch {
            '\n' => {
                segments.push(LineSegment {
                    text: &text[start..idx],
                    byte_start: start,
                    byte_end: idx + ch.len_utf8(),
                });
                start = idx + ch.len_utf8();
            }
            '\r' => {
                let mut end = idx + ch.len_utf8();
                if let Some((next_idx, '\n')) = iter.peek().copied() {
                    iter.next();
                    end = next_idx + '\n'.len_utf8();
                }
                segments.push(LineSegment {
                    text: &text[start..idx],
                    byte_start: start,
                    byte_end: end,
                });
                start = end;
            }
            _ => {}
        }
    }

    if start < text.len() {
        segments.push(LineSegment {
            text: &text[start..],
            byte_start: start,
            byte_end: text.len(),
        });
    }

    segments
}

pub fn line_range_extract(
    text: &str,
    start_line: usize,
    end_line: usize,
    line_base: usize,
    include_line_numbers: bool,
    include_fingerprint: bool,
) -> Result<LineRangeExtractResult, String> {
    if text.chars().count() > MAX_TEXT_LENGTH {
        return Err(format!(
            "Input length {} exceeds MAX_TEXT_LENGTH {}",
            text.chars().count(),
            MAX_TEXT_LENGTH
        ));
    }

    if start_line > end_line {
        return Err(format!(
            "start_line ({}) must be <= end_line ({})",
            start_line, end_line
        ));
    }

    let mut findings: Vec<LineExtractFinding> = vec![];

    let lines = split_line_segments(text);
    let total_lines = lines.len();

    let start_1based = line_number_to_one_based(start_line, line_base)?;
    let end_1based = line_number_to_one_based(end_line, line_base)?;

    let mut valid_range = true;
    let mut start_1based = start_1based;
    let mut end_1based = end_1based;

    if start_1based < 1 {
        valid_range = false;
        findings.push(LineExtractFinding {
            kind: "out_of_range".to_string(),
            message: format!("start_line {} is before the first line", start_line),
        });
        start_1based = 1;
    }

    if end_1based > total_lines {
        valid_range = false;
        findings.push(LineExtractFinding {
            kind: "out_of_range".to_string(),
            message: format!(
                "end_line {} exceeds total lines ({})",
                end_line, total_lines
            ),
        });
        end_1based = total_lines;
    }

    let mut extracted_lines: Vec<LineExtractLine> = vec![];
    let mut extracted_text_parts: Vec<String> = vec![];

    let start_idx = start_1based.saturating_sub(1);
    let end_idx = std::cmp::min(end_1based, lines.len());
    for i in start_idx..end_idx {
        let line_text = lines[i].text.to_string();
        let line_dict = if include_line_numbers {
            LineExtractLine {
                text: line_text,
                line: Some(i + line_base),
            }
        } else {
            LineExtractLine {
                text: line_text,
                line: None,
            }
        };
        extracted_lines.push(line_dict);
        extracted_text_parts.push(lines[i].text.to_string());
    }

    let (byte_start, byte_end) = if start_idx < end_idx && end_idx > 0 {
        (lines[start_idx].byte_start, lines[end_idx - 1].byte_end)
    } else {
        (text.len(), text.len())
    };
    let char_start = text[..byte_start].chars().count();
    let char_end = text[..byte_end].chars().count();

    let newline_style = detect_newline_style(text).to_string();
    let join_sep = match newline_style.as_str() {
        "CRLF" => "\r\n",
        "CR" => "\r",
        _ => "\n",
    };
    let extracted_text = extracted_text_parts.join(join_sep);

    let fingerprint = if include_fingerprint {
        fingerprint(&extracted_text)
    } else {
        String::new()
    };

    let ends_with_newline = text.ends_with('\n') || text.ends_with('\r');

    Ok(LineRangeExtractResult {
        line_count_total: total_lines,
        start_line,
        end_line,
        valid_range,
        text: extracted_text,
        lines: extracted_lines,
        byte_start,
        byte_end,
        char_start,
        char_end,
        newline_style,
        ends_with_newline,
        fingerprint,
        findings,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstDifference {
    pub line_offset: usize,
    pub line_number: usize,
    pub left: Option<String>,
    pub right: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRangeCompareResult {
    pub equal: bool,
    pub left_fingerprint: String,
    pub right_fingerprint: String,
    pub diff_summary: String,
    pub first_difference: Option<FirstDifference>,
}

pub fn line_range_compare(
    left_text: &str,
    right_text: &str,
    start_line: usize,
    end_line: usize,
    line_base: usize,
    comparison_mode: &str,
) -> Result<LineRangeCompareResult, String> {
    if left_text.chars().count() > MAX_TEXT_LENGTH {
        return Err(format!(
            "left_text length {} exceeds MAX_TEXT_LENGTH {}",
            left_text.chars().count(),
            MAX_TEXT_LENGTH
        ));
    }
    if right_text.chars().count() > MAX_TEXT_LENGTH {
        return Err(format!(
            "right_text length {} exceeds MAX_TEXT_LENGTH {}",
            right_text.chars().count(),
            MAX_TEXT_LENGTH
        ));
    }

    let valid_modes = ["exact", "ignore_trailing_whitespace", "normalize_newlines"];
    if !valid_modes.contains(&comparison_mode) {
        return Err(format!(
            "Invalid comparison_mode: {}. Use one of: {}",
            comparison_mode,
            valid_modes.join(", ")
        ));
    }

    let left_lines = split_line_segments(left_text);
    let right_lines = split_line_segments(right_text);

    let start_1based = line_number_to_one_based(start_line, line_base)?;
    let end_1based = line_number_to_one_based(end_line, line_base)?;

    if start_1based > left_lines.len() || start_1based > right_lines.len() {
        return Err(format!(
            "start_line ({}) exceeds available lines (left: {}, right: {})",
            start_line,
            left_lines.len(),
            right_lines.len()
        ));
    }

    let left_slice: Vec<&str> = {
        let start = start_1based.saturating_sub(1);
        let end = std::cmp::min(end_1based, left_lines.len());
        if start >= end {
            Vec::new()
        } else {
            left_lines[start..end]
                .iter()
                .map(|line| line.text)
                .collect()
        }
    };
    let right_slice: Vec<&str> = {
        let start = start_1based.saturating_sub(1);
        let end = std::cmp::min(end_1based, right_lines.len());
        if start >= end {
            Vec::new()
        } else {
            right_lines[start..end]
                .iter()
                .map(|line| line.text)
                .collect()
        }
    };

    fn normalize_for_compare(s: &str, mode: &str) -> String {
        match mode {
            "ignore_trailing_whitespace" => s.trim_end().to_string(),
            "normalize_newlines" => s.trim_end_matches('\r').to_string(),
            _ => s.to_string(),
        }
    }

    let left_norm: Vec<String> = left_slice
        .iter()
        .map(|l| normalize_for_compare(l, comparison_mode))
        .collect();
    let right_norm: Vec<String> = right_slice
        .iter()
        .map(|r| normalize_for_compare(r, comparison_mode))
        .collect();

    let equal = left_norm == right_norm;

    let left_text_slice = left_slice.join("\n");
    let right_text_slice = right_slice.join("\n");
    let left_fp = fingerprint(&left_text_slice);
    let right_fp = fingerprint(&right_text_slice);

    let mut diff_summary = if equal {
        "equal".to_string()
    } else {
        "different".to_string()
    };
    let mut first_diff: Option<FirstDifference> = None;

    if !equal {
        for (i, (l, r)) in left_norm.iter().zip(right_norm.iter()).enumerate() {
            if l != r {
                first_diff = Some(FirstDifference {
                    line_offset: i,
                    line_number: start_1based + i,
                    left: Some(left_slice[i].to_string()),
                    right: Some(right_slice[i].to_string()),
                });
                diff_summary = format!("differ at line {}", start_1based + i);
                break;
            }
        }
        if first_diff.is_none() && left_norm.len() != right_norm.len() {
            let min_len = std::cmp::min(left_norm.len(), right_norm.len());
            diff_summary = format!(
                "different lengths: {} vs {} lines",
                left_norm.len(),
                right_norm.len()
            );
            if min_len < std::cmp::max(left_norm.len(), right_norm.len()) {
                let idx = min_len;
                first_diff = Some(FirstDifference {
                    line_offset: idx,
                    line_number: start_1based + idx,
                    left: if idx < left_slice.len() {
                        Some(left_slice[idx].to_string())
                    } else {
                        None
                    },
                    right: if idx < right_slice.len() {
                        Some(right_slice[idx].to_string())
                    } else {
                        None
                    },
                });
            }
        }
    }

    Ok(LineRangeCompareResult {
        equal,
        left_fingerprint: left_fp,
        right_fingerprint: right_fp,
        diff_summary,
        first_difference: first_diff,
    })
}
