use std::collections::HashMap;
use std::sync::LazyLock;

pub static CONFUSABLES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m: HashMap<&'static str, &'static str> = HashMap::new();
    let data = include_str!("confusables_generated.rs");
    for line in data.lines() {
        let line = line.trim();
        if line.starts_with("m.insert(\"") && line.ends_with("\");") {
            let content = line.strip_prefix("m.insert(\"").unwrap();
            let content = content.strip_suffix("\");").unwrap();
            if let Some((key, value)) = content.split_once("\", \"") {
                m.insert(key, value);
            }
        }
    }
    m
});

pub fn has_confusables(text: &str) -> bool {
    text.chars().any(|c| {
        let key = format!("U+{:04X}", c as u32);
        CONFUSABLES.get(key.as_str()).is_some()
    })
}

pub fn find_confusables(text: &str) -> Vec<(char, &'static str)> {
    text.chars()
        .filter_map(|c| {
            let key = format!("U+{:04X}", c as u32);
            CONFUSABLES.get(key.as_str()).map(|sub| (c, *sub))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confusables_loaded() {
        assert!(CONFUSABLES.len() > 1400, "Confusables should have entries");
    }

    #[test]
    fn test_cyrillic_a_confusable() {
        let key = "U+0410";
        assert_eq!(CONFUSABLES.get(key), Some(&"U+0041"));
    }
}
