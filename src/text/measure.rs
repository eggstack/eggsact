use serde::{Deserialize, Serialize};
use unicode_general_category::get_general_category;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordMetrics {
    pub words: usize,
    pub unique_words_casefolded: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharCategoryMetrics {
    pub letters: usize,
    pub digits: usize,
    pub punctuation: usize,
    pub symbols: usize,
    pub spaces: usize,
    pub control_chars: usize,
    pub combining_marks: usize,
}

pub fn text_length(text: &str) -> usize {
    text.chars().count()
}

pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

pub fn word_metrics(text: &str) -> WordMetrics {
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|token| token.chars().any(|c| c.is_alphabetic()))
        .collect();
    let unique_words_casefolded: std::collections::BTreeSet<String> = words
        .iter()
        .map(|word| caseless::default_case_fold_str(word))
        .collect();

    WordMetrics {
        words: words.len(),
        unique_words_casefolded: unique_words_casefolded.len(),
    }
}

pub fn char_category_metrics(text: &str) -> CharCategoryMetrics {
    let mut letters = 0usize;
    let mut digits = 0usize;
    let mut punctuation = 0usize;
    let mut symbols = 0usize;
    let mut spaces = 0usize;
    let mut control_chars = 0usize;
    let mut combining_marks = 0usize;

    for c in text.chars() {
        let cat = get_general_category(c).abbreviation();
        match cat.as_bytes().first().copied().map(char::from) {
            Some('L') => letters += 1,
            Some('N') => digits += 1,
            Some('P') => punctuation += 1,
            Some('S') => symbols += 1,
            Some('Z') => spaces += 1,
            Some('C') if cat == "Cc" => control_chars += 1,
            Some('M') => combining_marks += 1,
            _ => {}
        }
    }

    CharCategoryMetrics {
        letters,
        digits,
        punctuation,
        symbols,
        spaces,
        control_chars,
        combining_marks,
    }
}

pub fn line_count(text: &str) -> usize {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized.lines().count()
}

pub fn char_frequency(text: &str) -> std::collections::HashMap<char, usize> {
    let mut freq = std::collections::HashMap::new();
    for c in text.chars() {
        *freq.entry(c).or_insert(0) += 1;
    }
    freq
}
