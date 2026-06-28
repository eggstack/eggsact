use fancy_regex::{Captures, Regex};
use std::collections::HashMap;
use std::sync::LazyLock;

static PCT_SYMBOL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)\s*%(?!\s*\d)").unwrap());
static PERCENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)\s*percent").unwrap());
static COMPLEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)\s*([+-])\s*(\d+(?:\.\d+)?)\s*i").unwrap());

// --- Cached alternation strings ---

static UNIT_ALT: LazyLock<String> = LazyLock::new(|| {
    // BUG-009 / parity B9: sort by length descending so compound units like
    // "km/h", "mi/h", "m/s" are tried before single-character units (m, h,
    // s) that would otherwise swallow the prefix.
    let mut keys: Vec<&str> = crate::calc::units::UNIT_ALIASES.keys().copied().collect();
    keys.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    keys.iter()
        .map(|u| regex_escape(u))
        .collect::<Vec<_>>()
        .join("|")
});

static NUMBER_WORD_ALT: LazyLock<String> = LazyLock::new(|| {
    NUMBER_WORDS
        .keys()
        .map(|w| regex_escape(w))
        .collect::<Vec<_>>()
        .join("|")
});

static TEMP_UNITS_LIST: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "f",
        "c",
        "k",
        "fahrenheit",
        "celsius",
        "kelvin",
        "rankine",
        "degf",
        "degc",
        "degk",
        "degr",
        "ra",
    ]
});

// --- Static regex patterns (no dynamic data) ---

static TEMP_CONVERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    let temp_unit = r"(?:[cfk]|celsius|fahrenheit|kelvin|rankine|degc|degf|degk|degr|ra)";
    // NOTE (BUG-007 / parity B7): \s* instead of \s+ so that compact forms
    // like "100c in f" trigger the canonicalization pass.
    Regex::new(&format!(
        r"(?i)(\d+(?:\.\d+)?)\s*({temp_unit})\s+(in|to|as|into)\s+({temp_unit})\b"
    ))
    .unwrap()
});

static BINARY_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(\d+(?:\.\d+)?|\((?:[^()]|\([^()]*\))*\))\s+(not|in|to|as|into)\s+(\d+(?:\.\d+)?)\b",
    )
    .unwrap()
});

static BINARY_WORD_INNER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(not|in|to|as|into)\b").unwrap());

static TO_THE_POWER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)\s+to\s+the\s+(\d+(?:\.\d+)?)").unwrap());

static DEGREES_NON_TEMP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)\s*(?:degrees?|deg)\s+(\w+)").unwrap());

static DEGREES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)\s*(?:degrees?|deg)\b").unwrap());

static POINT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?<=[\d)])\s*point\s*").unwrap());

static MERGE_DECIMAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+\.\d*)\s+(\d)").unwrap());

static IMPLICIT_MUL_FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
    let funcs = [
        "sqrt",
        "sin",
        "cos",
        "tan",
        "asin",
        "acos",
        "atan",
        "sinh",
        "cosh",
        "tanh",
        "log",
        "log10",
        "log2",
        "exp",
        "abs",
        "factorial",
        "cbrt",
        "floor",
        "ceil",
        "round",
        "sign",
        "mean",
        "median",
        "mode",
        "std",
        "variance",
        "gcd",
        "lcm",
        "perm",
        "comb",
        "isprime",
        "nextprime",
        "prevprime",
        "primefactors",
        "random",
        "randint",
        "gauss",
        "sum",
        "max",
        "min",
        "hypot",
        "clamp",
        "asinh",
        "acosh",
        "atanh",
    ];
    Regex::new(&format!(r"(\d|\))\s*({})\b", funcs.join("|"))).unwrap()
});

static IMPLICIT_MUL_PAREN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\))\s*(\d|\()").unwrap());

static IMPLICIT_MUL_DIGIT_PAREN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d)\s*\(").unwrap());

static FACTORIAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(\d+(?:\.\d+)?|\((?:[^()]*|\([^()]*\))*\)|[a-zA-Z_]\w*\((?:[^()]*|\([^()]*\))*\))(\!+)",
    )
    .unwrap()
});

static SPLIT_NUM_MINUS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+(?:-\d+)+$").unwrap());

static SPLIT_DOUBLE_MINUS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+--\d+").unwrap());

static SPLIT_TRAILING_MINUS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+-$").unwrap());

static UNIT_INLINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    // BUG-009 / parity B9: same shape but not anchored, used to scan the
    // joined post-split string for `<num>*<unit>` segments so that mixed-
    // unit arithmetic (e.g., "60 mph + 60 km/h" → "60*mph + 60*km/h") is
    // detected after the operator splitter runs.
    //
    // The unit class EXCLUDES `/` and `%` because they are operators
    // (division, modulo/percent). Without this, `50 / 5` would be misread
    // as `<num>=50, unit=/`, and `17 % 5` as `<num>=17, unit=%`.
    Regex::new(r"(?P<num>\d+(?:\.\d+)?)\*?(?P<unit>[a-zA-Z°]+(?:/[a-zA-Z°]+)?)").unwrap()
});

static SAME_UNIT_DIV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([a-zA-Z]+)/(\d+(?:\.\d+)?)\*([a-zA-Z]+)\b").unwrap());

static CONVERT_SIMPLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^convert\((\d+(?:\.\d+)?)\*([a-zA-Z°][a-zA-Z0-9°/]*),([a-zA-Z°][a-zA-Z0-9°/]*)\)$")
        .unwrap()
});

static CONVERT_BARE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^convert\((\d+(?:\.\d+)?),([a-zA-Z°][a-zA-Z0-9°/]*)\)$").unwrap()
});

static TEMP_HANDLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^temp\(([^,]+),([a-zA-Z°][a-zA-Z0-9°]*),([a-zA-Z°][a-zA-Z0-9°]*)\)$").unwrap()
});

// --- Dynamic regex patterns (depend on LazyLock data) ---

static HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    let nwa = &*NUMBER_WORD_ALT;
    Regex::new(&format!(r"\b({})-({})\b", nwa, nwa)).unwrap()
});

static NUMBER_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    let nwa = &*NUMBER_WORD_ALT;
    Regex::new(&format!("\\b({})\\b", nwa)).unwrap()
});

static UNIT_CARET_ATTACH_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ua = &*UNIT_ALT;
    Regex::new(&format!(r"(?i)(\d+(?:\.\d+)?)\s+({ua})\s*\^\s*([23])\b")).unwrap()
});

static UNIT_CARET_DENOM_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ua = &*UNIT_ALT;
    Regex::new(&format!(r"(?i)/\s*({ua})\s*\^\s*(\d+)\b")).unwrap()
});

static UNIT_CARET_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ua = &*UNIT_ALT;
    Regex::new(&format!(r"(?i)/\(\s*({ua})\s*\)\s*\^\s*(\d+)\b")).unwrap()
});

static UNIT_POWER_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ua = &*UNIT_ALT;
    Regex::new(&format!(r"(?i)\b({ua})\s+(squared|cubed)\b")).unwrap()
});

static UNIT_SPELLED_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ua = &*UNIT_ALT;
    // (BUG-009 / parity B9) Accept optional "*" between number and unit
    // so that the form inserted by BARE_SIMPLE_UNIT_RE / BARE_COMPOUND_UNIT_RE
    // also matches here.
    Regex::new(&format!(
        r"(?i)(\d+(?:\.\d+)?)\s*\*?\s*({ua})\s+(?:in|to)\s+({ua})\b"
    ))
    .unwrap()
});

static UNIT_COMPOUND_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ua = &*UNIT_ALT;
    Regex::new(&format!(
        r"(?i)(\d+(?:\.\d+)?)\s*\*?\s*({ua})\s*/\s*({ua})\s+(?:in|to)\s*({ua})\s*/\s*({ua})\b"
    ))
    .unwrap()
});

// --- BUG-009 / parity B9: bare compound / spaced unit handling ---
//
// Compound units (km/h, mi/h, m/s, …) contain a "/" which the operator
// splitter would otherwise turn into division, splitting "60 km/h" into
// ["60", "km", "/", "h"]. Insert "*" before the compound unit so the
// whole token survives tokenization.
//
// Single-unit spaced forms ("60 mph") become ["60", "mph"] after operator
// splitting and "mph" then collides with the evaluator's identifier table.
// Inserting "*" produces "60*mph" which the unit preprocessor can convert
// to the target unit when mixed-unit arithmetic follows.
//
// Both patterns run BEFORE the operator splitter via the normalize() pass.

static BARE_COMPOUND_UNITS: &[&str] = &[
    "km/h",
    "mi/h",
    "m/s",
    "km/s",
    "mi/s",
    "bit/s",
    "bits/s",
    "kilobit/s",
    "kilobits/s",
    "megabit/s",
    "megabits/s",
    "gigabit/s",
    "gigabits/s",
];

static BARE_COMPOUND_UNIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    let alt = BARE_COMPOUND_UNITS
        .iter()
        .map(|u| regex_escape(u))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"(?i)(\d+(?:\.\d+)?)(\s*)({alt})\b")).unwrap()
});

static SPLIT_UNIT_DIV_RE: LazyLock<Regex> = LazyLock::new(|| {
    // "<num> <u1> / <u2>" forms where u1/u2 are canonical rate units
    // (e.g. "60 km / h" -> "(60*km)/h" so the right-hand unit isn't
    // consumed as the Planck constant `h`).
    let pairs = [
        ("km", "h"),
        ("km", "hr"),
        ("km", "hour"),
        ("km", "hours"),
        ("mi", "h"),
        ("mi", "hr"),
        ("mi", "hour"),
        ("mi", "hours"),
        ("m", "s"),
        ("m", "sec"),
        ("m", "second"),
        ("m", "seconds"),
        ("km", "s"),
        ("km", "sec"),
        ("km", "second"),
        ("km", "seconds"),
        ("mi", "s"),
        ("mi", "sec"),
        ("mi", "second"),
        ("mi", "seconds"),
        ("km", "min"),
        ("km", "minute"),
        ("km", "minutes"),
        ("mi", "min"),
        ("mi", "minute"),
        ("mi", "minutes"),
    ];
    let alt = pairs
        .iter()
        .map(|(a, b)| format!(r"({})\s+/\s+({})", regex_escape(a), regex_escape(b)))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"(?i)(\d+(?:\.\d+)?)\s*(?:{})", alt)).unwrap()
});

static PER_UNIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    // "<num> <distance> per <time>" forms (e.g. "60 kilometer per hour",
    // "60 miles per hour") -> "(60*km)/h" so the rate expression survives
    // tokenization.
    Regex::new(
        r"(?i)(\d+(?:\.\d+)?)\s*(kilometers?|kilometres?|miles?|meters?|metres?|feet|foot|inches?|inch|km|mi|yd|ft)\s+per\s+(hours?|hr|minutes?|min|seconds?|sec|h|s)\b",
    ).unwrap()
});

static BARE_SIMPLE_UNIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    // BUG-009 / parity B9: Spaced `<num> <unit>` forms (e.g. "60 mph",
    // "100 m", "50 ft") become "<num>*<unit>" so the unit survives operator
    // splitting. The list deliberately skips ambiguous one-letter
    // identifiers in identifier-only contexts; single-letter units are
    // accepted because they're unambiguous when preceded by a number with
    // whitespace (Python parity).
    let units = [
        // Speed / velocity
        "mph",
        "kph",
        "knot",
        "knots",
        "ft/s",
        "ft/min",
        "ft/h",
        "yd/s",
        "yd/min",
        "yd/h",
        "mi/s",
        "mi/min",
        "mi/h",
        "km/s",
        "km/min",
        "km/h",
        "m/s",
        "m/min",
        "m/h",
        "mach",
        // Length
        "m",
        "cm",
        "mm",
        "km",
        "ft",
        "yd",
        "mi",
        "in",
        "inch",
        "inches",
        "foot",
        "feet",
        "mile",
        "miles",
        "meter",
        "meters",
        "metre",
        "metres",
        "yard",
        "yards",
        "rod",
        "fathom",
        // Mass
        "kg",
        "g",
        "mg",
        "lb",
        "lbs",
        "oz",
        "ton",
        "tonne",
        // Time
        "s",
        "sec",
        "secs",
        "second",
        "seconds",
        "min",
        "mins",
        "minute",
        "minutes",
        "h",
        "hr",
        "hrs",
        "hour",
        "hours",
        "day",
        "days",
        "week",
        "weeks",
        "year",
        "years",
        // Volume
        "l",
        "L",
        "ml",
        "liter",
        "liters",
        "litre",
        "litres",
        "gal",
        "qt",
        "pt",
        "cup",
        // Energy / power
        "j",
        "J",
        "kJ",
        "kj",
        "cal",
        "kcal",
        "wh",
        "kWh",
        "kwh",
        "w",
        "W",
        "kw",
        "KW",
        "hp",
        // Pressure
        "pa",
        "Pa",
        "kpa",
        "KPa",
        "kPa",
        "mpa",
        "MPa",
        "psi",
        "bar",
        "atm",
        "torr",
        "mmHg",
        // Temperature (long forms; short f/c/k handled elsewhere)
        "celsius",
        "fahrenheit",
        "kelvin",
        "rankine",
        "degc",
        "degf",
        "degk",
        "degr",
        // Data
        "bit",
        "bits",
        "byte",
        "bytes",
        "kb",
        "mb",
        "gb",
        "tb",
    ];
    let alt = {
        // Sort by length descending so longer units (e.g., "miles") are
        // tried before shorter prefixes ("mile", "mi", "m"). Without this,
        // the regex engine would greedily match "m" inside "miles" and
        // misread expressions like "10 miles" as "10*m ...".
        let mut sorted = units.to_vec();
        sorted.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        sorted
            .iter()
            .map(|u| regex_escape(u))
            .collect::<Vec<_>>()
            .join("|")
    };
    Regex::new(&format!(r"(?i)(\d+(?:\.\d+)?)(\s+)({})\b", alt)).unwrap()
});

// --- Loop patterns (cached per-entry) ---

static TEMP_DEG_CONV_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    TEMP_UNITS_LIST
        .iter()
        .map(|tu| {
            Regex::new(&format!(
                r"(?i)(\d+(?:\.\d+)?)\s*(?:degrees?|deg)\s+(?:in|to|as|into)\s+{}\b",
                regex_escape(tu)
            ))
            .unwrap()
        })
        .collect()
});

static TEMP_DEG_UNIT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    TEMP_UNITS_LIST
        .iter()
        .map(|tu| {
            Regex::new(&format!(
                r"(?i)(\d+(?:\.\d+)?)\s*(?:degrees?|deg)\s+{}\b",
                regex_escape(tu)
            ))
            .unwrap()
        })
        .collect()
});

static DIGIT_SCALE_PATTERNS: LazyLock<Vec<(Regex, f64)>> = LazyLock::new(|| {
    let scales: &[(&str, &str)] = &[
        ("hundred", "100"),
        ("thousand", "1000"),
        ("million", "1000000"),
        ("billion", "1000000000"),
        ("trillion", "1000000000000"),
        ("quadrillion", "1000000000000000"),
        ("quintillion", "1000000000000000000"),
    ];
    scales
        .iter()
        .map(|(word, val)| {
            let re = Regex::new(&format!(r"\b(\d+(?:\.\d+)?)\s*{}\b", regex_escape(word))).unwrap();
            (re, val.parse::<f64>().unwrap())
        })
        .collect()
});

static CONSTANT_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let mut entries: Vec<_> = CONSTANT_WORDS.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(phrase, _)| std::cmp::Reverse(phrase.len()));
    entries
        .iter()
        .map(|(phrase, canonical)| {
            (
                Regex::new(&word_boundary_regex(phrase)).unwrap(),
                **canonical,
            )
        })
        .collect()
});

static MULTI_WORD_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    MULTI_WORD_NUMBERS
        .iter()
        .map(|(phrase, replacement)| {
            (
                Regex::new(&word_boundary_regex(phrase)).unwrap(),
                *replacement,
            )
        })
        .collect()
});

static STRIPPED_LONG_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    STRIPPED_PHRASES
        .iter()
        .filter(|p| p.len() > 10)
        .map(|p| Regex::new(&word_boundary_regex(p)).unwrap())
        .collect()
});

static STRIPPED_SHORT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    STRIPPED_PHRASES
        .iter()
        .filter(|p| p.len() <= 10)
        .map(|p| Regex::new(&word_boundary_regex(p)).unwrap())
        .collect()
});

static OPERATOR_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let mut entries: Vec<_> = OPERATOR_CONVERSIONS.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(word, _)| std::cmp::Reverse(word.len()));
    entries
        .iter()
        .map(|(word, symbol)| (Regex::new(&word_boundary_regex(word)).unwrap(), **symbol))
        .collect()
});

static FUNC_NAME_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let mut entries: Vec<_> = FUNCTION_MAPPINGS.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
    entries
        .iter()
        .map(|(name, standard)| (Regex::new(&word_boundary_regex(name)).unwrap(), **standard))
        .collect()
});

static COMPACT_FUNC_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    let funcs: Vec<&str> = FUNCTION_MAPPINGS
        .values()
        .copied()
        .filter(|name| {
            !name.ends_with(|c: char| c.is_ascii_digit()) && *name != "log10" && *name != "log2"
        })
        .collect();
    if funcs.is_empty() {
        None
    } else {
        Some(
            Regex::new(&format!(
                r"(?<![A-Za-z_])({})\s*([+-]?\d+(?:\.\d+)?)",
                funcs
                    .iter()
                    .map(|f| regex_escape(f))
                    .collect::<Vec<_>>()
                    .join("|")
            ))
            .unwrap(),
        )
    }
});

static FUNC_FIX_PATTERNS: LazyLock<Vec<(Regex, bool, &'static str)>> = LazyLock::new(|| {
    let multi_arg_funcs: &[&str] = &[
        "mean", "median", "mode", "std", "variance", "var", "gcd", "lcm", "perm", "comb", "nPr",
        "nCr", "sum", "max", "min", "clamp", "gauss", "hypot", "randint",
    ];
    FUNCTION_MAPPINGS
        .values()
        .copied()
        .filter_map(|func| {
            let escaped = regex_escape(func);
            let is_multi = multi_arg_funcs.contains(&func);
            let pattern = if is_multi {
                format!(r"\b{0}\s*\*\s*([^()]+)", escaped)
            } else {
                format!(r"\b{0}\s*\*\s*([^()+\-]*)", escaped)
            };
            Regex::new(&pattern).ok().map(|re| (re, is_multi, func))
        })
        .collect()
});

fn regex_escape(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '\\' | '(' | ')' | '[' | ']' | '{' | '}' | '.' | '*' | '+' | '?' | '^' | '$' | '|' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

const MAX_TEXT_LENGTH: usize = 10_000;

#[doc(hidden)]
pub static NUMBER_WORDS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Single digits
    m.insert("zero", "0");
    m.insert("one", "1");
    m.insert("two", "2");
    m.insert("three", "3");
    m.insert("four", "4");
    m.insert("five", "5");
    m.insert("six", "6");
    m.insert("seven", "7");
    m.insert("eight", "8");
    m.insert("nine", "9");

    // Teens
    m.insert("ten", "10");
    m.insert("eleven", "11");
    m.insert("twelve", "12");
    m.insert("thirteen", "13");
    m.insert("fourteen", "14");
    m.insert("fifteen", "15");
    m.insert("sixteen", "16");
    m.insert("seventeen", "17");
    m.insert("eighteen", "18");
    m.insert("nineteen", "19");

    // Tens
    m.insert("twenty", "20");
    m.insert("thirty", "30");
    m.insert("forty", "40");
    m.insert("fifty", "50");
    m.insert("sixty", "60");
    m.insert("seventy", "70");
    m.insert("eighty", "80");
    m.insert("ninety", "90");

    // Multipliers
    m.insert("hundred", "100");
    m.insert("thousand", "1000");
    m.insert("million", "1000000");
    m.insert("billion", "1000000000");
    m.insert("trillion", "1000000000000");
    m.insert("quadrillion", "1000000000000000");
    m.insert("quintillion", "1000000000000000000");

    // Fractions
    m.insert("half", "0.5");
    m.insert("quarter", "0.25");
    m.insert("thousandth", "0.001");
    m.insert("millionth", "0.000001");
    m.insert("billionth", "0.000000001");

    m
});

/// Multi-word fraction numbers (e.g., "one half" → "0.5").
/// Applied before individual number word replacement.
#[doc(hidden)]
pub static MULTI_WORD_NUMBERS: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    vec![
        ("one half", "0.5"),
        ("one quarter", "0.25"),
        ("one third", "0.3333333333333333"),
        ("two thirds", "0.6666666666666666"),
        ("three quarters", "0.75"),
    ]
});

#[doc(hidden)]
pub static OPERATOR_CONVERSIONS: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert("plus", "+");
        m.insert("positive", "+");
        m.insert("minus", "-");
        m.insert("negative", "-");
        m.insert("times", "*");
        m.insert("multiplied by", "*");
        m.insert("divided by", "/");
        m.insert("over", "/");
        m.insert("per", "/");
        m.insert("divide", "/");
        m.insert("raised to the power of", "**");
        m.insert("raised to", "**");
        m.insert("to the power of", "**");
        m.insert("mod", "%");
        m.insert("modulo", "%");
        m.insert("remainder", "%");

        m.insert("bitand", "&");
        m.insert("bit and", "&");
        m.insert("bitor", "|");
        m.insert("bit or", "|");
        m.insert("or", "|");
        m.insert("bitxor", "^");
        m.insert("bit xor", "^");
        m.insert("xor", "^");
        m.insert("bitnot", "~");
        m.insert("bit not", "~");
        m.insert("not", "~");
        m.insert("left shift", "<<");
        m.insert("shift left", "<<");
        m.insert("lshift", "<<");
        m.insert("right shift", ">>");
        m.insert("shift right", ">>");
        m.insert("rshift", ">>");
        m.insert("of", "*");
        m.insert("in", "IN");
        m.insert("into", "IN");
        m.insert("to", "TO");
        m.insert("as", "TO");

        m
    });

#[doc(hidden)]
pub static FUNCTION_MAPPINGS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Trigonometric
    m.insert("sine", "sin");
    m.insert("cosine", "cos");
    m.insert("tangent", "tan");
    m.insert("arc sine", "asin");
    m.insert("arcsine", "asin");
    m.insert("arcsin", "asin");
    m.insert("arc cosine", "acos");
    m.insert("arccos", "acos");
    m.insert("arccosine", "acos");
    m.insert("arc tangent", "atan");
    m.insert("arctan", "atan");
    m.insert("arctangent", "atan");
    m.insert("hyperbolic sine", "sinh");
    m.insert("hyperbolic cosine", "cosh");
    m.insert("hyperbolic tangent", "tanh");
    m.insert("arcsinh", "asinh");
    m.insert("arccosh", "acosh");
    m.insert("arctanh", "atanh");
    m.insert("inverse sine", "asin");
    m.insert("inverse cosine", "acos");
    m.insert("inverse tangent", "atan");
    m.insert("inverse hyperbolic sine", "asinh");
    m.insert("inverse hyperbolic cosine", "acosh");
    m.insert("inverse hyperbolic tangent", "atanh");
    m.insert("arc cos", "acos");
    m.insert("arc sin", "asin");
    m.insert("arc tan", "atan");
    m.insert("hyperbolic arcsine", "asinh");
    m.insert("hyperbolic arccosine", "acosh");
    m.insert("hyperbolic arctangent", "atanh");

    // Logarithmic
    m.insert("logarithm", "log");
    m.insert("natural log", "log");
    m.insert("natural logarithm", "log");
    m.insert("ln", "log");
    m.insert("log base ten", "log10");
    m.insert("log ten", "log10");
    m.insert("log two", "log2");
    m.insert("log base two", "log2");

    // Power/Root
    m.insert("square root", "sqrt");
    m.insert("cube root", "cbrt");
    m.insert("root", "sqrt");

    // Rounding/Absolute
    m.insert("absolute value", "abs");
    m.insert("abs value", "abs");
    m.insert("absolute", "abs");
    m.insert("magnitude", "abs");
    m.insert("ceiling", "ceil");

    // Factorial/Combinatorics
    m.insert("fact", "factorial");
    m.insert("factorial", "factorial");
    m.insert("nPr", "nPr");
    m.insert("nCr", "nCr");

    // Statistical
    m.insert("average", "mean");
    m.insert("stdev", "std");
    m.insert("gcd", "gcd");
    m.insert("lcm", "lcm");
    m.insert("perm", "perm");
    m.insert("comb", "comb");

    // Percentage
    m.insert("percent_of", "percentof");
    m.insert("as_percent", "aspercent");

    // Prime
    m.insert("is_prime", "isprime");
    m.insert("prime_factors", "primefactors");
    m.insert("next_prime", "nextprime");
    m.insert("prev_prime", "prevprime");

    // Self-mappings (function name maps to itself)
    m.insert("convert", "convert");
    m.insert("temp", "temp");
    m.insert("floor", "floor");
    m.insert("trunc", "trunc");
    m.insert("sign", "sign");
    m.insert("degrees", "degrees");
    m.insert("radians", "radians");
    m.insert("hypot", "hypot");
    m.insert("round", "round");
    m.insert("pow", "pow");
    m.insert("atan2", "atan2");
    m.insert("log1p", "log1p");
    m.insert("expm1", "expm1");
    m.insert("uniform", "uniform");
    m.insert("cbrt", "cbrt");
    m.insert("sqrt", "sqrt");
    m.insert("log", "log");
    m.insert("log10", "log10");
    m.insert("log2", "log2");
    m.insert("abs", "abs");
    m.insert("exp", "exp");
    m.insert("ceil", "ceil");
    m.insert("clamp", "clamp");
    m.insert("sin", "sin");
    m.insert("cos", "cos");
    m.insert("tan", "tan");
    m.insert("asin", "asin");
    m.insert("acos", "acos");
    m.insert("atan", "atan");
    m.insert("sinh", "sinh");
    m.insert("cosh", "cosh");
    m.insert("tanh", "tanh");
    m.insert("asinh", "asinh");
    m.insert("acosh", "acosh");
    m.insert("atanh", "atanh");

    // Aggregate functions
    m.insert("mean", "mean");
    m.insert("median", "median");
    m.insert("mode", "mode");
    m.insert("std", "std");
    m.insert("std_sample", "std_sample");
    m.insert("stds", "std_sample");
    m.insert("variance", "variance");
    m.insert("var", "var");
    m.insert("variance_sample", "variance_sample");
    m.insert("vars", "vars");
    m.insert("var_sample", "var_sample");
    m.insert("sum", "sum");
    m.insert("max", "max");
    m.insert("min", "min");
    m.insert("product", "product");

    // Complex number functions
    m.insert("real", "real");
    m.insert("imag", "imag");
    m.insert("conj", "conj");
    m.insert("conjugate", "conj");
    m.insert("phase", "phase");
    m.insert("polar", "polar");
    m.insert("rect", "rect");

    // Bitwise functions
    m.insert("bitand", "bitand");
    m.insert("bitor", "bitor");
    m.insert("bitxor", "bitxor");
    m.insert("bitnot", "bitnot");
    m.insert("bitlshift", "bitlshift");
    m.insert("bitrshift", "bitrshift");

    // Memory functions
    m.insert("store", "store");
    m.insert("recall", "recall");
    m.insert("Mplus", "Mplus");
    m.insert("Mminus", "Mminus");
    m.insert("MC", "MC");
    m.insert("MR", "MR");
    m.insert("M", "MR");
    m.insert("setvar", "setvar");
    m.insert("getvar", "getvar");
    m.insert("delvar", "delvar");
    m.insert("listvars", "listvars");
    m.insert("clearvars", "clearvars");

    // Random functions
    m.insert("random", "random");
    m.insert("randint", "randint");
    m.insert("randrange", "randrange");
    m.insert("randn", "randn");
    m.insert("gauss", "gauss");
    m.insert("seed", "seed");

    m
});

#[doc(hidden)]
pub static CONSTANT_WORDS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("avogadro number", "na");
    m.insert("avogadro", "na");
    m.insert("avogadros", "na");
    m.insert("gas constant", "r");
    m.insert("molar gas constant", "r");
    m.insert("ideal gas constant", "r");
    m.insert("planck constant", "planckconstant");
    m.insert("planck", "planckconstant");
    m.insert("boltzmann constant", "k");
    m.insert("boltzmann", "k");
    m.insert("speed of light", "c");
    m.insert("speed of light in vacuum", "c");
    m.insert("c zero", "c");
    m.insert("elementary charge", "elementarycharge");
    m.insert("e charge", "elementarycharge");
    m.insert("faraday constant", "f");
    m.insert("faraday", "f");
    m.insert("atomic mass", "u");
    m.insert("atomic mass unit", "u");
    m.insert("amu", "u");
    m.insert("vacuum permittivity", "epsilon0");
    m.insert("permittivity of free space", "epsilon0");
    m.insert("vacuum permeability", "mu0");
    m.insert("permeability of free space", "mu0");
    m.insert("magnetic constant", "mu0");
    m.insert("standard gravity", "standardgravity");
    m.insert("gravity", "standardgravity");
    m.insert("earth gravity", "standardgravity");
    m.insert("gravitational constant", "gravitationalconstant");
    m.insert("newton constant", "gravitationalconstant");
    m.insert("big g", "G");
    m.insert("electron mass", "me");
    m.insert("proton mass", "mp");
    m.insert("neutron mass", "mn");
    m.insert("classical electron radius", "re");
    m.insert("electron radius", "re");
    m.insert("fine structure constant", "alpha");
    m.insert("sommerfeld", "alpha");
    m.insert("rydberg constant", "rydberg");
    m.insert("stefan boltzmann", "stefan");
    m.insert("stefan-boltzmann constant", "stefan");
    m.insert("wien constant", "wien");
    m.insert("wien displacement", "wien");
    m
});

#[doc(hidden)]
pub static STRIPPED_PHRASES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "what's",
        "what is",
        "a",
        "?",
        "calculate",
        "compute",
        "tell me",
        "give me",
        "the ",
        "please ",
        "hey ",
        "hi ",
        "can you ",
        "could you ",
        "would you ",
        "i want to know ",
        "i'd like to know ",
        "what's the value of ",
        "what's the result of ",
        "what is the value of ",
        "what is the result of ",
        "the value of ",
        "the result of ",
        "the answer is ",
        "and ",
    ]
});

fn word_boundary_regex(word: &str) -> String {
    format!(r"\b{}\b", regex_escape(word))
}

/// Combine consecutive number words in a string.
///
/// After number word replacement, "twenty one" becomes "20 1". This function
/// scans for sequences of space-separated numbers and combines them:
/// - "20 1" -> "21" (tens + ones)
/// - "100 2 3" -> "123" (hundreds + tens + ones)
/// - "1 100 20 2" -> "1*100+22" (single + compound)
fn combine_consecutive_number_words(input: &str) -> String {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return input.to_string();
    }

    // Find runs of consecutive numeric tokens (possibly with spaces between them)
    let mut result = Vec::new();
    let mut i = 0;

    while i < parts.len() {
        // Check if this part is a number (possibly with leading/trailing sign)
        if let Some(first_num) = parse_simple_number(parts[i]) {
            // Collect the run of numbers
            let mut run: Vec<(String, f64)> = vec![(parts[i].to_string(), first_num)];
            let mut j = i + 1;
            while j < parts.len() {
                if let Some(n) = parse_simple_number(parts[j]) {
                    run.push((parts[j].to_string(), n));
                    j += 1;
                } else {
                    break;
                }
            }

            if run.len() > 1 {
                // We have a run of numbers to combine
                let combined = combine_number_run(&run);
                for token in combined {
                    result.push(token);
                }
            } else {
                result.push(parts[i].to_string());
            }
            i = j;
        } else {
            result.push(parts[i].to_string());
            i += 1;
        }
    }

    result.join(" ")
}

/// Try to parse a simple integer or float from a token.
fn parse_simple_number(token: &str) -> Option<f64> {
    let s = token.trim_start_matches('+');
    s.parse::<f64>().ok()
}

/// Combine a run of numbers into final tokens.
///
/// Uses the same logic as Python's combine_number_parts:
/// - Tens + ones combine: [20, 2] -> ["22"]
/// - Hundreds chain with multiplication: [3, 100, 20, 2] -> ["3", "*", "100", "+", "22"]
/// - Simple additions stay as-is: [5, 3] -> ["5", "+", "3"]
fn combine_number_run(run: &[(String, f64)]) -> Vec<String> {
    if run.is_empty() {
        return vec![];
    }

    let values: Vec<f64> = run.iter().map(|(_, v)| *v).collect();

    // Check if this is a compound number (has tens >= 20 or hundreds)
    let has_compound = values
        .iter()
        .any(|&v| v >= 100.0 || ((20.0..100.0).contains(&v) && v % 10.0 == 0.0));

    if has_compound && values.len() > 1 {
        combine_number_parts(&values)
    } else {
        // Simple addition: keep as separate numbers with + between
        let mut result = Vec::new();
        for (i, (orig, _)) in run.iter().enumerate() {
            if i > 0 {
                result.push("+".to_string());
            }
            result.push(orig.clone());
        }
        result
    }
}

/// Combine number parts into a single numeric token.
///
/// English cardinal numbers are built from an accumulating group below the
/// current large scale:
/// - [20, 2] -> 22
/// - [3, 100, 20, 2] -> 322
/// - [1, 100, 20, 1, 1000] -> 121000
fn combine_number_parts(values: &[f64]) -> Vec<String> {
    if values.is_empty() {
        return vec![];
    }

    let mut total = 0.0;
    let mut group = 0.0;

    for &part in values {
        if part == 100.0 {
            if group == 0.0 {
                group = 1.0;
            }
            group *= part;
        } else if part >= 1000.0 {
            if group == 0.0 {
                group = 1.0;
            }
            total += group * part;
            group = 0.0;
        } else {
            group += part;
        }
    }

    vec![format_number(total + group)]
}

/// Format a number for output, showing integers without decimal point.
fn format_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// NZ-2: Canonicalize compact lowercase temperature conversion phrases.
///
/// Lowercase c/f/k are accepted as temperature units by the unit preprocessor,
/// but conversion words are replaced before token handling. A phrase like
/// "100 c in f" otherwise collapses into "100cINf" and the conversion detector
/// never sees separate source/target unit tokens.
fn normalize_lowercase_temperature_conversion(expr: &str) -> String {
    TEMP_CONVERSION_RE
        .replace_all(expr, |caps: &Captures| {
            let from_unit = caps[2].to_uppercase();
            let to_unit = caps[4].to_uppercase();
            let conv_word = caps[3].to_uppercase();
            format!("{} {} {} {}", &caps[1], from_unit, conv_word, to_unit)
        })
        .to_string()
}

/// NZ-7: Check for ambiguous binary word usage like "5 not 6" or "1 in 2".
/// These words are reserved for unary bitwise NOT or unit conversion. When
/// they appear between two numeric values, the meaning is ambiguous.
fn binary_word_check(expr: &str) -> Result<(), String> {
    if let Some(m) = BINARY_WORD_RE.find(expr).unwrap() {
        let matched = m.as_str();
        if let Some(wm) = BINARY_WORD_INNER_RE.find(matched).unwrap() {
            let word = wm.as_str().to_lowercase();
            return Err(format!(
                "Syntax error: '{}' is not a binary operator in this context. \
                 Use parentheses for unary 'not' (e.g., '~(5+6)'); \
                 for unit conversion, follow the pattern '<value> in <unit>'.",
                word
            ));
        }
    }
    Ok(())
}

pub fn normalize(expr: &str) -> Result<String, String> {
    if expr.len() > MAX_TEXT_LENGTH {
        return Err(format!("Input exceeds {} characters", MAX_TEXT_LENGTH));
    }

    // M16: Replace unicode math operators with ASCII equivalents BEFORE lowercasing
    let mut result = expr.replace('\u{00d7}', "*"); // × → *
    result = result.replace('\u{00f7}', "/"); // ÷ → /
    result = result.replace('\u{2212}', "-"); // − → -

    let result = result.to_lowercase();

    // NZ-7: Binary word validation - check before any word replacement
    binary_word_check(&result)?;

    // NZ-2: Normalize lowercase temperature conversions before operator word replacement
    let result = normalize_lowercase_temperature_conversion(&result);

    // M18: Convert hyphens between number words to spaces
    // e.g., "twenty-one" -> "twenty one" (prevents hyphen being treated as minus)
    let mut result = HYPHEN_RE.replace_all(&result, "$1 $2").to_string();

    // Strip long filler phrases first (e.g., "what's the value of", "what is the result of")
    // These must be stripped before operator conversion to avoid partial matches
    for re in STRIPPED_LONG_PATTERNS.iter() {
        result = re.replace_all(&result, "").to_string();
    }

    // M19: Handle "N to the M" power phrases BEFORE "to" is converted to operator
    result = TO_THE_POWER_RE.replace_all(&result, "$1**$2").to_string();

    // NZ-8 + M24: Handle degrees → radian conversion BEFORE operator word replacement
    // Skip conversion for temperature units: "100 degrees in fahrenheit" stays as temperature
    // Temperature units that should NOT trigger angle conversion
    // First handle "N degrees in/to <temp_unit>" - keep as temperature, strip degrees
    for tu_re in TEMP_DEG_CONV_PATTERNS.iter() {
        result = tu_re.replace_all(&result, "$1").to_string();
    }
    // Handle "N degrees <temp_unit>" (without in/to keyword) - keep as temperature
    for tu_re in TEMP_DEG_UNIT_PATTERNS.iter() {
        result = tu_re.replace_all(&result, "$1").to_string();
    }
    // Handle "N degrees <non-temp-unit>" → convert angle to radians
    // e.g., "5 degrees radians" → "(5*pi/180)*radian"
    result = DEGREES_NON_TEMP_RE
        .replace_all(&result, "($1*pi/180)*$2")
        .to_string();
    // Then handle remaining "N degrees" → radian conversion
    result = DEGREES_RE.replace_all(&result, "($1*pi/180)").to_string();

    // NZ-4: Normalize spaced unit caret exponents: "5 m ^ 2" → "5 m2"
    // "5 m ^ 2" → "5 m2" (attached quantity with unit)
    result = UNIT_CARET_ATTACH_RE
        .replace_all(&result, |caps: &Captures| {
            let unit = &caps[2];
            let power = &caps[3];
            format!("{} {}{}", &caps[1], unit, power)
        })
        .to_string();
    // "/ m ^ 2" → "/m**2" (denominator form)
    result = UNIT_CARET_DENOM_RE
        .replace_all(&result, |caps: &Captures| {
            let unit = caps[1].to_lowercase();
            format!("/{}**{}", unit, &caps[2])
        })
        .to_string();
    // "/(m) ^ 2" → "/(m)**2" (parenthesized denominator)
    result = UNIT_CARET_PAREN_RE
        .replace_all(&result, |caps: &Captures| {
            let unit = caps[1].to_lowercase();
            format!("/({})**{}", unit, &caps[2])
        })
        .to_string();

    // M23: Handle "N thousand" scale words (evaluate to product)
    for (re, sv) in DIGIT_SCALE_PATTERNS.iter() {
        result = re
            .replace_all(&result, |caps: &Captures| {
                let v: f64 = caps[1].parse().unwrap_or(0.0);
                let product = v * sv;
                if product.fract() == 0.0 && product.abs() < 1e15 {
                    format!("{}", product as i64)
                } else {
                    format!("{}", product)
                }
            })
            .to_string();
    }

    // Handle "N%" -> N/100
    result = PCT_SYMBOL_RE.replace_all(&result, "($1/100)").to_string();

    // Handle "N percent" -> N/100
    result = PERCENT_RE.replace_all(&result, "($1/100)").to_string();

    // Convert constant phrases (e.g., "gas constant" -> "R")
    // Sort by phrase length descending so multi-word phrases match before shorter ones
    for (re, canonical) in CONSTANT_PATTERNS.iter() {
        result = re.replace_all(&result, *canonical).to_string();
    }

    // NZ-6: Replace multi-word fraction numbers before individual word replacement
    // e.g., "one half" -> "0.5", "two thirds" -> "0.666..."
    for (re, replacement) in MULTI_WORD_PATTERNS.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }

    // Convert number words
    result = NUMBER_WORD_RE
        .replace_all(&result, |caps: &Captures| {
            NUMBER_WORDS
                .get(caps.get(1).map(|m| m.as_str()).unwrap_or(""))
                .copied()
                .unwrap_or("")
        })
        .to_string();

    // M20: Handle "point" as decimal separator: "5 point 3" -> "5.3"
    // Only when preceded by a digit or ')'
    // NOTE (BUG-006 / parity B6): This MUST run before
    // `combine_consecutive_number_words()` so that digit words following a
    // decimal point are absorbed into the fractional part rather than being
    // treated as a separate number ("three point one four" -> "3.14",
    // not "3.1 + 4").
    result = POINT_RE.replace_all(&result, ".").to_string();

    // Merge digits following a decimal point: "3.1 4" -> "3.14"
    // After "point" replacement, space-separated digit words after the decimal
    // become separate tokens. Iteratively concatenate them into a single
    // decimal number.
    let mut prev_result = String::new();
    while prev_result != result {
        prev_result = result.clone();
        result = MERGE_DECIMAL_RE.replace_all(&result, "$1$2").to_string();
    }

    // Combine consecutive number words: "twenty one" -> "21", "one hundred twenty two" -> "122"
    result = combine_consecutive_number_words(&result);

    // BUG-009 / parity B9: "kilometer per hour" / "miles per hour" forms.
    // Must run BEFORE the operator-word replacement (which converts "per"
    // into "/") and BEFORE the bare-unit "*" insertion below so the full
    // phrase is collapsed into a canonical compound unit first.
    result = PER_UNIT_RE
        .replace_all(&result, |caps: &Captures| {
            let num = &caps[1];
            let distance = caps[2].to_lowercase();
            let time = caps[3].to_lowercase();
            let dist_unit: &str = match distance.as_str() {
                "kilometer" | "kilometers" | "kilometre" | "kilometres" | "km" => "km",
                "mile" | "miles" | "mi" => "mi",
                "meter" | "meters" | "metre" | "metres" | "m" => "m",
                "foot" | "feet" | "ft" => "ft",
                "inch" | "inches" => "in",
                "yard" | "yards" | "yd" => "yd",
                _ => return caps[0].to_string(),
            };
            let time_unit: &str = match time.as_str() {
                "hour" | "hours" | "hr" | "h" => "h",
                "minute" | "minutes" | "min" => "min",
                "second" | "seconds" | "sec" | "s" => "s",
                _ => return caps[0].to_string(),
            };
            format!("{}*{}/{}", num, dist_unit, time_unit)
        })
        .to_string();

    // BUG-009 / parity B9: "<num> <unit1> / <unit2>" split-rate forms.
    // "60 km / h" -> "(60*km)/h" so the right-hand unit isn't consumed as
    // the Planck constant `h` by the evaluator.
    result = SPLIT_UNIT_DIV_RE
        .replace_all(&result, |caps: &Captures| {
            let num = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
            let u1 = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let u2 = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            // Emit "<num>*<u1>/<u2>" without parens so the inline regex
            // can still see the full compound unit when scanning the joined
            // expression. BUG-009 / parity B9.
            format!("{}*{}/{}", num, u1, u2)
        })
        .to_string();

    // BUG-009 / parity B9: bare compound unit forms. Insert "*" between
    // number and a compound unit ("60 km/h" -> "60*km/h") so the operator
    // splitter doesn't turn the embedded "/" into division.
    result = BARE_COMPOUND_UNIT_RE
        .replace_all(&result, "$1*$3")
        .to_string();

    // BUG-009 / parity B9: bare simple unit forms. Insert "*" between a
    // number and a known spaced unit ("60 mph" -> "60*mph") so the token
    // survives operator splitting. Limited to long unit names to avoid
    // false positives with short identifiers.
    result = BARE_SIMPLE_UNIT_RE
        .replace_all(&result, "$1*$3")
        .to_string();

    // M17: Split compact function forms: "sin30" -> "sin 30"
    // Guard names that already end in digits so "log10" remains as-is
    if let Some(ref compact_re) = *COMPACT_FUNC_RE {
        result = compact_re.replace_all(&result, "$1 $2").to_string();
    }

    // Convert operator words - sort by length descending to ensure longer patterns match first
    for (re, symbol) in OPERATOR_PATTERNS.iter() {
        result = re.replace_all(&result, *symbol).to_string();
    }

    // Convert function names - sort by length descending to ensure longer patterns match first
    for (re, standard) in FUNC_NAME_PATTERNS.iter() {
        result = re.replace_all(&result, *standard).to_string();
    }

    // Fix "func * expr" patterns caused by "of" → "*" conversion
    // Convert "cbrt * 8" → "cbrt(8)" for known function names
    // For multi-arg functions like "mean * 1+2+3", convert +/- to commas: "mean(1,2,3)"
    for (re, is_multi, func) in FUNC_FIX_PATTERNS.iter() {
        result = re
            .replace_all(&result, |caps: &Captures| {
                let arg = caps[1].trim();
                if arg.is_empty() {
                    format!("{} *", func)
                } else if *is_multi {
                    // Replace top-level +/- with commas, preserving signs on numbers
                    // e.g., "1+2+3" → "1,2,3", "-1+2" → "-1,2"
                    let mut comma_parts = Vec::new();
                    let mut current = String::new();
                    let mut paren_depth = 0;
                    let chars: Vec<char> = arg.chars().collect();
                    let mut i = 0;
                    while i < chars.len() {
                        match chars[i] {
                            '(' => {
                                paren_depth += 1;
                                current.push(chars[i]);
                                i += 1;
                            }
                            ')' => {
                                paren_depth -= 1;
                                current.push(chars[i]);
                                i += 1;
                            }
                            '+' | '-' if paren_depth == 0 && !current.is_empty() => {
                                // Check if this +/- is a sign (after operator or at start) or separator
                                let last_non_space = current.trim_end().chars().last();
                                match last_non_space {
                                    Some('(') | Some('+') | Some('-') | Some('*') | Some('/')
                                    | Some(',') => {
                                        // This is a sign character, not a separator
                                        current.push(chars[i]);
                                        i += 1;
                                    }
                                    _ => {
                                        // This is a separator
                                        let trimmed = current.trim().to_string();
                                        if !trimmed.is_empty() {
                                            comma_parts.push(trimmed);
                                        }
                                        current = String::new();
                                        // Include the sign as part of the next number
                                        if chars[i] == '-' {
                                            current.push('-');
                                        }
                                        i += 1;
                                    }
                                }
                            }
                            _ => {
                                current.push(chars[i]);
                                i += 1;
                            }
                        }
                    }
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        comma_parts.push(trimmed);
                    }
                    format!("{}({})", func, comma_parts.join(","))
                } else {
                    format!("{}({})", func, arg)
                }
            })
            .to_string();
    }

    // NZ-9 + M25: Postfix unit power words: "m squared" -> "m2", "cm cubed" -> "cm3"
    // Uses full UNIT_ALIASES to match any known unit before "squared"/"cubed"
    result = UNIT_POWER_RE
        .replace_all(&result, |caps: &Captures| {
            let unit = &caps[1];
            let power = if caps[2].eq_ignore_ascii_case("squared") {
                "2"
            } else {
                "3"
            };
            format!("{}{}", unit, power)
        })
        .to_string();

    // BUG-009 / parity B9 patterns moved earlier in the pipeline (see the
    // pass inserted after `combine_consecutive_number_words()` above) so
    // they run before the operator-word replacement consumes "per".

    // NZ-10 + M26: Spelled unit conversions with full unit list
    // "30 km/h in mph" -> "convert(30*km/h,mph)"
    result = UNIT_SPELLED_RE
        .replace_all(&result, "convert($1*$2,$3)")
        .to_string();

    // NZ-10 + M27: Compound unit conversions: "60mi/h in m/s" -> "convert(60*mi/h,m/s)"
    result = UNIT_COMPOUND_RE
        .replace_all(&result, |caps: &Captures| {
            let num = &caps[1];
            let from_num = &caps[2];
            let from_den = &caps[3];
            let to_num = &caps[4];
            let to_den = &caps[5];
            format!(
                "convert({}*{}/{},{}/{})",
                num, from_num, from_den, to_num, to_den
            )
        })
        .to_string();

    // Strip short filler phrases AFTER operator conversion (e.g., "the ", "please ")
    for re in STRIPPED_SHORT_PATTERNS.iter() {
        result = re.replace_all(&result, "").to_string();
    }

    // M20 (relocated): "point" and decimal-merge have already been applied
    // before `combine_consecutive_number_words()` above (BUG-006 / parity B6).

    // Handle complex numbers: 3+4i -> 3+4j, 3 + 4i -> 3+4j, 3 + 4 i -> 3+4j
    result = COMPLEX_RE.replace_all(&result, "($1$2$3j)").to_string();

    // Normalize whitespace
    let mut result = result.split_whitespace().collect::<Vec<_>>().join(" ");

    // M22: Insert implicit * between adjacent tokens (5sin -> 5*sin, (2+3)(4) -> (2+3)*(4))
    // Handle: digit/)/(func, func/digit/)(, )(, digit)(

    // Insert * between digit/ ) and function name: "5sin" -> "5*sin", "(2+3)sin" -> "(2+3)*sin"
    result = IMPLICIT_MUL_FUNC_RE
        .replace_all(&result, "$1*$2")
        .to_string();

    // Insert * between ) and digit/(: "(2+3)4" -> "(2+3)*4", "(2+3)(4+5)" -> "(2+3)*(4+5)"
    result = IMPLICIT_MUL_PAREN_RE
        .replace_all(&result, "$1*$2")
        .to_string();

    // Insert * between digit and (: "3(4+5)" -> "3*(4+5)"
    result = IMPLICIT_MUL_DIGIT_PAREN_RE
        .replace_all(&result, "$1*(")
        .to_string();

    // NZ-13 + M21: Factorial postfix: "5!" -> "factorial(5)"
    // Iteratively handle nested factorial: "5!!" -> "factorial(5)!" -> "factorial(factorial(5))"
    // Also handles func(args)! e.g. "factorial(5)!" -> "factorial(factorial(5))"
    let mut prev_fact = String::new();
    while prev_fact != result {
        prev_fact = result.clone();
        result = FACTORIAL_RE
            .replace_all(&result, |caps: &Captures| {
                let content = &caps[1];
                let bangs = &caps[2];
                let mut out = format!("factorial({})", content);
                if bangs.len() > 1 {
                    out.push_str(&"!".repeat(bangs.len() - 1));
                }
                out
            })
            .to_string();
    }

    Ok(result)
}

pub fn split_at_operators(expr: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0;
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '(' | '[' => {
                paren_depth += 1;
                current.push(ch);
                i += 1;
            }
            ')' | ']' => {
                paren_depth -= 1;
                current.push(ch);
                i += 1;
            }
            '+' | '-' if paren_depth == 0 => {
                // Use negative lookbehind to preserve "e+" in scientific notation
                if ch == '+' {
                    let is_scientific = !current.is_empty()
                        && matches!(current.chars().last(), Some('e') | Some('E'));
                    if is_scientific {
                        current.push(ch);
                        i += 1;
                        continue;
                    }
                }
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                }
                tokens.push(ch.to_string());
                current = String::new();
                i += 1;
            }
            '/' | '%' if paren_depth == 0 => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                }
                tokens.push(ch.to_string());
                current = String::new();
                i += 1;
            }
            '*' | '^' if paren_depth == 0 => {
                // Handle ** (power operator)
                if ch == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
                    if !current.trim().is_empty() {
                        tokens.push(current.trim().to_string());
                    }
                    tokens.push("**".to_string());
                    current = String::new();
                    i += 2;
                    continue;
                }
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                }
                tokens.push(ch.to_string());
                current = String::new();
                i += 1;
            }
            _ => {
                current.push(ch);
                i += 1;
            }
        }
    }

    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }

    // Post-split cleanup: handle edge cases the main loop leaves behind.
    // Walk with a while-loop so newly-inserted tokens get re-checked.
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].clone();
        let is_op = token == "+"
            || token == "-"
            || token == "/"
            || token == "%"
            || token == "*"
            || token == "**"
            || token == "^";

        if !is_op {
            // Case 1: "4-5-3" (multiple subtractions in one token)
            if should_split_number_minus(&token) {
                let parts: Vec<&str> = token.splitn(2, '-').collect();
                tokens[i] = parts[0].to_string();
                tokens.insert(i + 1, "-".to_string());
                tokens.insert(i + 2, parts[1].to_string());
                continue; // re-check the new token at i (now parts[0])
            }
            // Case 2: "5-" trailing minus (e.g., "5-(3+2)" → "5-" then "(3+2)")
            else if should_split_trailing_minus(&token) {
                let num_part = &token[..token.len() - 1];
                tokens[i] = num_part.to_string();
                tokens.insert(i + 1, "-".to_string());
                continue; // re-check
            }
            // Case 3: "4--5" (double minus → subtraction of negative)
            else if should_split_double_minus(&token) {
                let parts: Vec<&str> = token.splitn(2, "--").collect();
                tokens[i] = parts[0].to_string();
                tokens.insert(i + 1, "-".to_string());
                tokens.insert(i + 2, format!("-{}", parts[1]));
                // Don't advance — re-check the new token at i
                continue;
            }
            // Case 4: Space-separated number sequences
            else if should_split_number_sequence(&token) {
                let parts: Vec<&str> = token.split_whitespace().collect();
                let mut new_tokens: Vec<String> = Vec::new();
                for (pi, part) in parts.iter().enumerate() {
                    if pi > 0 {
                        new_tokens.push("+".to_string());
                    }
                    new_tokens.push(part.to_string());
                }
                tokens.splice(i..i + 1, new_tokens);
                continue; // re-check
            }
        }
        i += 1;
    }

    tokens
}

/// Check if token matches pattern: digit-sequence minus digit-sequence (e.g., "4-5-3").
fn should_split_number_minus(token: &str) -> bool {
    SPLIT_NUM_MINUS_RE.is_match(token).unwrap_or(false)
}

/// Check if token matches pattern: digit-sequence -- digit-sequence (e.g., "4--5").
fn should_split_double_minus(token: &str) -> bool {
    SPLIT_DOUBLE_MINUS_RE.is_match(token).unwrap_or(false)
}

/// Check if token ends with a trailing minus (e.g., "5-" when followed by parenthesized expr).
fn should_split_trailing_minus(token: &str) -> bool {
    SPLIT_TRAILING_MINUS_RE.is_match(token).unwrap_or(false)
}

/// Check if token is a space-separated number sequence (e.g., "3 100 20 2").
fn should_split_number_sequence(token: &str) -> bool {
    if !token.contains(' ') {
        return false;
    }
    let parts: Vec<&str> = token.split_whitespace().collect();
    if parts.len() < 2 {
        return false;
    }
    for part in &parts {
        let stripped = part.trim_start_matches('+').trim_start_matches('-');
        if stripped.is_empty() {
            return false;
        }
        // Must be a number (integer or float, possibly scientific notation)
        if stripped.parse::<f64>().is_err() {
            return false;
        }
    }
    true
}

#[doc(hidden)]
pub fn preprocess_units(tokens: &[String]) -> (Vec<String>, Option<String>) {
    // BUG-009 / parity B9: After split_at_operators(), unit-bearing tokens
    // like "60*mph" may have been broken into ["60", "*", "mph"]. We need to
    // operate on the joined string so the regex can see contiguous
    // "<num>*<unit>" segments.

    let joined = tokens.join("");

    // First pass: find the first unit segment in the joined string. Search
    // the inline regex (for normal units) first, then check for the percent
    // operator as a unit suffix only when it isn't followed by a digit (to
    // avoid misreading `17 % 5` as `<num>=17, unit=%`).
    let mut detected_unit: Option<String> = UNIT_INLINE_RE
        .captures(&joined)
        .ok()
        .flatten()
        .and_then(|caps| {
            let unit = caps.name("unit").map(|m| m.as_str()).unwrap_or("");
            // Accept known aliases OR compound forms (e.g. "mi/min"). Python
            // preserves "1 mi/min" verbatim even though mi/min isn't in its
            // UNIT_ALIASES table.
            if let Some(canon) = resolve_unit_alias(unit) {
                return Some(canon);
            }
            if unit.contains('/') {
                return Some(unit.to_string());
            }
            None
        });

    if detected_unit.is_none() {
        // Look for percent as a unit suffix: `<num>%` not followed by a
        // digit (so we don't catch `17 % 5` modulo).
        if let Some(idx) = joined.find('%') {
            // Walk backwards from `%` to confirm it's preceded by digits.
            let prefix = &joined[..idx];
            if prefix.ends_with(|c: char| c.is_ascii_digit() || c == '.') {
                // Confirm next char (if any) is NOT a digit.
                let after = joined.as_bytes().get(idx + 1).copied().unwrap_or(b' ');
                if !after.is_ascii_digit() {
                    detected_unit = Some("%".to_string());
                }
            }
        }
    }

    let target_unit = match detected_unit.clone() {
        Some(u) => u,
        None => return (tokens.to_vec(), None),
    };

    // Second pass: convert every "<num>*<unit>" segment to the target unit.
    // We rewrite the joined string, leaving non-unit text untouched.
    let rewritten = UNIT_INLINE_RE.replace_all(&joined, |caps: &Captures| {
        let num_str = caps.name("num").map(|m| m.as_str()).unwrap_or("0");
        let unit = caps.name("unit").map(|m| m.as_str()).unwrap_or("");
        match resolve_unit_alias(unit) {
            None => caps[0].to_string(),
            Some(canon) if canon == target_unit => num_str.to_string(),
            Some(canon) => {
                // BUG-009 / parity B9: preserve compound units (e.g.,
                // "mi/min", "km/s") that aren't tracked in UNIT_ALIASES as
                // plain display strings rather than mangling them. Python
                // parity preserves "1 mi/min" verbatim.
                if canon.contains('/')
                    && !crate::calc::units::UNIT_ALIASES.contains_key(canon.as_str())
                {
                    return caps[0].to_string();
                }
                match crate::calc::units::get_conversion_factor(&canon, &target_unit) {
                    Ok(factor) => {
                        if let Ok(num) = num_str.parse::<f64>() {
                            let converted = num * factor;
                            if converted.fract() == 0.0 && converted.abs() < 1e15 {
                                format!("{}", converted as i64)
                            } else {
                                format!("{}", converted)
                            }
                        } else {
                            num_str.to_string()
                        }
                    }
                    Err(_) => caps[0].to_string(),
                }
            }
        }
    });

    // Re-tokenize the rewritten string so downstream consumers (which expect
    // a Vec<String>) see the converted values.
    let mut re_tokens = split_at_operators(&rewritten);

    // If target_unit is "%", strip "%" from the rewritten string because
    // the evaluator doesn't understand unit suffixes for percent; the
    // caller's run() will append "%" to the formatted value.
    if target_unit == "%" {
        re_tokens.retain(|t| t != "%");
    }

    (re_tokens, Some(target_unit))
}

fn resolve_unit_alias(unit: &str) -> Option<String> {
    if let Some(canon) = crate::calc::units::UNIT_ALIASES.get(unit) {
        return Some(canon.to_string());
    }
    let lower = unit.to_lowercase();
    if let Some(canon) = crate::calc::units::UNIT_ALIASES.get(lower.as_str()) {
        return Some(canon.to_string());
    }
    None
}

/// Result of run - tuple of (string_representation, type_name)
pub type RunResult = (String, String);

/// Error type for `run()` that distinguishes evaluation errors from other errors.
#[derive(Debug, Clone)]
pub enum RunError {
    /// An error during expression evaluation (parse error, division by zero, etc.)
    Evaluation(String),
    /// An error during normalization or other processing
    Internal(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Evaluation(msg) => write!(f, "{}", msg),
            RunError::Internal(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for RunError {}

/// NZ-3: Wrap the denominator in parentheses for unit-on-division-right.
///
/// Detects patterns like "5*m/3*s" or "10*km/5*hr" where the right
/// operand of a division has a trailing unit, and wraps the entire
/// right operand in parens to preserve correct operator precedence.
pub fn add_same_unit_division_parens(expr: &str) -> String {
    SAME_UNIT_DIV_RE
        .replace_all(expr, |caps: &Captures| {
            let left_unit = &caps[1];
            let denom = &caps[2];
            let right_unit = &caps[3];
            format!("{}/({}*{})", left_unit, denom, right_unit)
        })
        .to_string()
}

/// Run natural language expression through the full pipeline.
///
/// This function normalizes the input (converting number words to digits,
/// operator words to symbols), tokenizes it, and evaluates the result.
///
/// # Arguments
///
/// * `expr` - A string slice containing the natural language expression
///
/// # Returns
///
/// * `Ok((String, String))` - The result and type
/// * `Err(RunError)` - An error if processing fails, distinguished by type
///
/// # Examples
///
/// ```
/// use eggsact::calc::run;
///
/// assert_eq!(run("thirty plus five").unwrap(), ("35".to_string(), "int".to_string()));
/// assert_eq!(run("two to the power of ten").unwrap(), ("1024".to_string(), "int".to_string()));
/// ```
pub fn run(expr: &str) -> Result<RunResult, RunError> {
    let normalized = normalize(expr).map_err(RunError::Internal)?;

    // Handle convert() and temp() patterns before evaluation
    if let Some(result) = handle_convert_pattern(&normalized) {
        return result.map_err(RunError::Evaluation);
    }
    if let Some(result) = handle_temp_pattern(&normalized) {
        return result.map_err(RunError::Evaluation);
    }

    let tokens = split_at_operators(&normalized);
    let (tokens, detected_unit) = preprocess_units(&tokens);
    let processed = tokens.join("");

    // NZ-3: Wrap denominator in parens for unit-on-division-right patterns
    // "5*m/3*s" → "5*m/(3*s)" so trailing units bind to the right operand
    let processed = add_same_unit_division_parens(&processed);

    let (value, value_type) =
        crate::calc::evaluator::evaluate(&processed).map_err(RunError::Evaluation)?;
    if let Some(unit) = detected_unit {
        Ok((format!("{} {}", value, unit), value_type))
    } else {
        Ok((value, value_type))
    }
}

/// Format a numeric result for display (integer if whole, float otherwise).
fn format_numeric_result(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// Handle `convert(value*unit, target_unit)` patterns.
///
/// After normalization, expressions like "30 km/h in mph" become
/// `convert(30*km/h,mph)`. This function detects and handles those
/// patterns before they reach the evaluator (which doesn't know about units).
fn handle_convert_pattern(normalized: &str) -> Option<Result<RunResult, String>> {
    // Match: convert(NUM*UNIT/TARGET_UNIT,TARGET) or convert(NUM*UNIT,TARGET)
    // The value part can be: NUM*UNIT, NUM*UNIT/UNIT, or just NUM
    if let Ok(Some(caps)) = CONVERT_SIMPLE_RE.captures(normalized) {
        let num_str = &caps[1];
        let from_unit = &caps[2];
        let to_unit = &caps[3];

        let value = num_str.parse::<f64>().ok()?;
        return Some(handle_convert_value(value, from_unit, to_unit));
    }

    // Also match: convert(NUM, target_unit) — dimensionless value
    if let Ok(Some(caps)) = CONVERT_BARE_RE.captures(normalized) {
        let num_str = &caps[1];
        let to_unit = &caps[2];

        let value = num_str.parse::<f64>().ok()?;
        // Dimensionless to unit: just try to create the unit
        return Some(Ok((
            format!("{} {}", format_numeric_result(value), to_unit),
            "float".to_string(),
        )));
    }

    None
}

/// Resolve a unit symbol to its canonical form using `UNIT_ALIASES`, tolerating
/// case mismatches (e.g., "CELSIUS", "Celsius", "celsius"). Mirrors the
/// case-folding done by `convert_temperature`.
fn resolve_unit_canon(unit: &str) -> String {
    if let Some(canon) = crate::calc::units::UNIT_ALIASES.get(unit).copied() {
        return canon.to_string();
    }
    let lower = unit.to_lowercase();
    if let Some(canon) = crate::calc::units::UNIT_ALIASES
        .get(lower.as_str())
        .copied()
    {
        return canon.to_string();
    }
    unit.to_string()
}

/// Perform the actual unit conversion for convert(value, from_unit, to_unit).
fn handle_convert_value(value: f64, from_unit: &str, to_unit: &str) -> Result<RunResult, String> {
    // Check if from_unit or to_unit are temperature units
    let from_resolved = resolve_unit_canon(from_unit);
    let to_resolved = resolve_unit_canon(to_unit);

    let from_is_temp = crate::calc::units::UNIT_BASE
        .get(from_resolved.as_str())
        .map(|d| d.category == "temperature")
        .unwrap_or(false);
    let to_is_temp = crate::calc::units::UNIT_BASE
        .get(to_resolved.as_str())
        .map(|d| d.category == "temperature")
        .unwrap_or(false);

    if from_is_temp && to_is_temp {
        let converted =
            crate::calc::units::convert_temperature(value, &from_resolved, &to_resolved)?;
        Ok((
            format!("{} {}", format_numeric_result(converted), &to_resolved),
            "float".to_string(),
        ))
    } else {
        let factor = crate::calc::units::get_conversion_factor(&from_resolved, &to_resolved)?;
        let converted = value * factor;
        Ok((
            format!("{} {}", format_numeric_result(converted), &to_resolved),
            "float".to_string(),
        ))
    }
}

/// Handle `temp(value, from_unit, to_unit)` patterns.
fn handle_temp_pattern(normalized: &str) -> Option<Result<RunResult, String>> {
    if let Ok(Some(caps)) = TEMP_HANDLE_RE.captures(normalized) {
        let value_str = &caps[1];
        let from_unit = &caps[2];
        let to_unit = &caps[3];

        // Try to parse the value as a number
        if let Ok(value) = value_str.parse::<f64>() {
            match crate::calc::units::convert_temperature(value, from_unit, to_unit) {
                Ok(converted) => {
                    return Some(Ok((
                        format!("{} {}", format_numeric_result(converted), to_unit),
                        "float".to_string(),
                    )));
                }
                Err(e) => {
                    return Some(Err(e));
                }
            }
        }
    }

    None
}
