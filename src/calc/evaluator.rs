use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

#[derive(Debug)]
pub enum EvaluationError {
    ParseError(String),
    DivisionByZero,
    InvalidOperation(String),
    UnknownFunction(String),
    UnknownConstant(String),
    StackOverflow,
    ValueOverflow,
}

impl std::fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError(s) => write!(f, "Parse error: {}", s),
            Self::DivisionByZero => write!(f, "Division by zero"),
            Self::InvalidOperation(s) => write!(f, "Invalid operation: {}", s),
            Self::UnknownFunction(s) => write!(f, "Unknown function: {}", s),
            Self::UnknownConstant(s) => write!(f, "Unknown constant: {}", s),
            Self::StackOverflow => write!(f, "Stack overflow (expression too nested)"),
            Self::ValueOverflow => write!(f, "Value overflow"),
        }
    }
}

const MAX_NESTING_DEPTH: usize = 100;
const MAX_EXPONENT: f64 = 10_000.0;
const MAX_RESULT_VALUE: f64 = 1e308;
const MAX_RESULT_DIGITS: usize = 10000;
const MAX_SHIFT_COUNT: usize = 50_000;
const MAX_INPUT_LENGTH: usize = 10_000;
// BUG-001 fix / parity B1: Python's `math.factorial` accepts arbitrarily
// large n. Rust's f64-based `factorial()` overflows past 170, but the
// Python reference returns a 309-digit int for factorial(170) and supports
// much larger n via big-integer arithmetic. 1000 is a practical upper
// bound that keeps the big-int multiplication cost bounded.
const MAX_FACTORIAL: i64 = 1000;
const MAX_PRIME: i64 = 1_000_000_000_000; // 10^12
const MAX_PERM_COMB: i64 = 10_000;

#[doc(hidden)]
pub static CONSTANTS: LazyLock<HashMap<&'static str, f64>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Mathematical constants
    m.insert("pi", std::f64::consts::PI);
    m.insert("e", std::f64::consts::E);
    m.insert("tau", std::f64::consts::TAU);
    m.insert("phi", 1.618_033_988_749_895);

    // Speed of light and vacuum constants
    m.insert("c", 299792458.0);
    m.insert("c0", 299792458.0);
    m.insert("speedoflight", 299792458.0);
    m.insert("speedoflightvacuum", 299792458.0);
    m.insert("epsilon0", 8.8541878128e-12);
    m.insert("vacuumpermittivity", 8.8541878128e-12);
    m.insert("mu0", 1.25663706212e-6);
    m.insert("vacuumpermeability", 1.25663706212e-6);

    // Planck constants
    m.insert("planck", 6.62607015e-34);
    m.insert("planckconstant", 6.62607015e-34);
    m.insert("hbar", 1.054571817e-34);
    m.insert("planckbar", 1.054571817e-34);
    m.insert("reducedplanck", 1.054571817e-34);

    // Boltzmann and gas constants
    m.insert("k", 1.380649e-23);
    m.insert("boltzmann", 1.380649e-23);
    m.insert("boltzmannconstant", 1.380649e-23);
    m.insert("R", 8.314462618);
    m.insert("r", 8.314462618);
    m.insert("gasconstant", 8.314462618);
    m.insert("idealgasconstant", 8.314462618);

    // Gravitational constant
    m.insert("G", 6.67430e-11);
    m.insert("gravitationalconstant", 6.67430e-11);

    // Avogadro
    m.insert("na", 6.02214076e23);
    m.insert("avogadro", 6.02214076e23);
    m.insert("avogadros", 6.02214076e23);

    // Elementary charge and electron
    m.insert("qe", 1.602176634e-19);
    m.insert("echarge", 1.602176634e-19);
    m.insert("elementarycharge", 1.602176634e-19);
    m.insert("me", 9.1093837015e-31);
    m.insert("electronmass", 9.1093837015e-31);
    m.insert("re", 2.8179403262e-15);
    m.insert("electronradius", 2.8179403262e-15);

    // Proton and neutron mass
    m.insert("mp", 1.67262192369e-27);
    m.insert("protonmass", 1.67262192369e-27);
    m.insert("mn", 1.67493e-27);
    m.insert("neutronmass", 1.67493e-27);

    // Atomic mass unit
    m.insert("u", 1.6605390666e-27);
    m.insert("amu", 1.6605390666e-27);
    m.insert("atomicmassunit", 1.6605390666e-27);

    // Fine structure constant
    m.insert("alpha", 0.0072973525693);
    m.insert("finestructure", 0.0072973525693);

    // Rydberg constant
    m.insert("rydberg", 10973731.56816);
    m.insert("rydbergconstant", 10973731.56816);

    // Stefan-Boltzmann constant
    m.insert("stefan", 5.670374419e-08);
    m.insert("stefanboltzmann", 5.670374419e-08);

    // Wien displacement constant
    m.insert("wien", 0.002897771955);
    m.insert("wienconstant", 0.002897771955);

    // Faraday constant
    m.insert("f", 96485.33212);
    m.insert("faraday", 96485.33212);
    m.insert("faradayconstant", 96485.33212);

    // Standard gravity
    m.insert("standardgravity", 9.80665);

    m
});

/// Result of evaluate - tuple of (string_representation, type_name)
pub type EvaluateResult = (String, String);

// ─── MCP-safe mode (matches Python's _mcp_mode + allow_random/allow_side_effects) ──

/// Functions whose output is non-deterministic (depend on global random state).
/// When `allow_random` is false, calls to these are rejected.
static RANDOM_FUNCTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut s = HashSet::new();
    for &name in &[
        "random",
        "randint",
        "randrange",
        "uniform",
        "randn",
        "gauss",
        "seed",
    ] {
        s.insert(name);
    }
    s
});

/// Functions that mutate evaluator state across calls (memory registers,
/// user variables). When `allow_side_effects` is false, calls to these are rejected.
static SIDE_EFFECT_FUNCTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut s = HashSet::new();
    for &name in &[
        "store",
        "recall",
        "m",
        "mplus",
        "mminus",
        "mc",
        "mr",
        "setvar",
        "getvar",
        "delvar",
        "listvars",
        "clearvars",
    ] {
        s.insert(name);
    }
    s
});

/// Global MCP mode flag. Set to true on first MCP request to match Python's
/// idempotent one-time check. When true, config loading is skipped and the
/// evaluator rejects random/side-effect functions.
static MCP_MODE: AtomicBool = AtomicBool::new(false);

/// When false, calls to random functions are rejected.
static ALLOW_RANDOM: AtomicBool = AtomicBool::new(true);

/// When false, calls to side-effect functions are rejected.
static ALLOW_SIDE_EFFECTS: AtomicBool = AtomicBool::new(true);

/// Maximum number of user variables (cap to prevent unbounded growth).
const MAX_USER_VARIABLES: usize = 1000;

/// Memory registers for store/recall/mplus/mminus/mc/mr.
static MEMORY_REGISTERS: LazyLock<Mutex<HashMap<String, f64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// User variables for setvar/getvar/delvar/listvars/clearvars.
static USER_VARIABLES: LazyLock<Mutex<HashMap<String, f64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// PRNG state (xorshift64) for random functions.
static PRNG_STATE: LazyLock<Mutex<u64>> = LazyLock::new(|| Mutex::new(123456789));

/// Box-Muller spare value for randn/gauss.
static GAUSS_SPARE: LazyLock<Mutex<Option<f64>>> = LazyLock::new(|| Mutex::new(None));

/// Enter MCP-safe mode. Idempotent: safe to call multiple times.
/// Sets `_mcp_mode = true` and disables random/side-effect functions.
pub fn set_mcp_mode() {
    if MCP_MODE.swap(true, Ordering::SeqCst) {
        return; // already configured
    }
    ALLOW_RANDOM.store(false, Ordering::SeqCst);
    ALLOW_SIDE_EFFECTS.store(false, Ordering::SeqCst);
}

/// Returns true if the evaluator is in MCP-safe mode.
pub fn is_mcp_mode() -> bool {
    MCP_MODE.load(Ordering::SeqCst)
}

/// Check whether a function name is allowed in the current mode.
/// Returns Ok(()) if allowed, or Err with an appropriate message.
fn check_function_allowed(name: &str) -> Result<(), EvaluationError> {
    let lower = name.to_lowercase();
    if !ALLOW_RANDOM.load(Ordering::SeqCst) && RANDOM_FUNCTIONS.contains(lower.as_str()) {
        return Err(EvaluationError::InvalidOperation(format!(
            "Function '{}' is non-deterministic and is disabled in MCP mode (allow_random=false)",
            name
        )));
    }
    if !ALLOW_SIDE_EFFECTS.load(Ordering::SeqCst) && SIDE_EFFECT_FUNCTIONS.contains(lower.as_str())
    {
        return Err(EvaluationError::InvalidOperation(format!(
            "Function '{}' mutates evaluator state and is disabled in MCP mode (allow_side_effects=false)",
            name
        )));
    }
    Ok(())
}

/// Evaluate a mathematical expression and return the result as a string with type info.
pub fn evaluate(expr: &str) -> Result<EvaluateResult, String> {
    let expr = expr.trim();
    if expr.len() > MAX_INPUT_LENGTH {
        return Err(format!("Input exceeds {} characters", MAX_INPUT_LENGTH));
    }
    match parse_expression(expr, &mut 0) {
        Ok(result) => format_result(result),
        Err(EvaluationError::InvalidOperation(ref s)) if s.starts_with("__string_result__") => {
            let value = s.trim_start_matches("__string_result__").to_string();
            Ok((value, "str".to_string()))
        }
        Err(EvaluationError::InvalidOperation(ref s)) if s.starts_with("__int_result__") => {
            // BUG-002 / parity B2: big-integer results (factorial, perm)
            // surface as Python's "int" type via a sentinel error.
            let value = s.trim_start_matches("__int_result__").to_string();
            Ok((value, "int".to_string()))
        }
        Err(e) => Err(e.to_string()),
    }
}

fn format_result(result: f64) -> Result<EvaluateResult, String> {
    if result.is_nan() || result.is_infinite() {
        Err(EvaluationError::ValueOverflow.to_string())
    } else if result.fract() == 0.0 && result >= i64::MIN as f64 && result <= i64::MAX as f64 {
        Ok((format!("{}", result as i64), "int".to_string()))
    } else {
        Ok((format!("{}", result), "float".to_string()))
    }
}

fn check_result_value(v: f64) -> Result<f64, EvaluationError> {
    if v.is_nan() || v.is_infinite() || v.abs() > MAX_RESULT_VALUE {
        Err(EvaluationError::ValueOverflow)
    } else {
        // Check if integer part has too many digits
        if v.abs() >= 1.0 && v.abs() < i64::MAX as f64 {
            let int_part = v.abs() as i64;
            let digits = int_part.to_string().len();
            if digits > MAX_RESULT_DIGITS {
                return Err(EvaluationError::ValueOverflow);
            }
        }
        Ok(v)
    }
}

// ─── Tokens ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Multiply,
    Divide,
    FloorDiv,
    Power,
    Modulo,
    BitXor,
    BitAnd,
    BitOr,
    BitNot,
    LShift,
    RShift,
    LeftParen,
    RightParen,
    Comma,
    Identifier(String),
}

// ─── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(expr: &str) -> Result<Vec<Token>, EvaluationError> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' | '\n' | '\r' => continue,

            // Numbers: decimal, hex, octal, binary, scientific
            '0'..='9' => {
                tokens.push(tokenize_number(ch, &mut chars)?);
            }
            '.' => {
                // .5 style — peek to see if followed by digit
                if chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                    tokens.push(tokenize_number(ch, &mut chars)?);
                } else {
                    return Err(EvaluationError::ParseError(
                        "Unexpected character: .".to_string(),
                    ));
                }
            }

            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    tokens.push(Token::Power);
                } else {
                    tokens.push(Token::Multiply);
                }
            }
            '/' => {
                if chars.peek() == Some(&'/') {
                    chars.next();
                    tokens.push(Token::FloorDiv);
                } else {
                    tokens.push(Token::Divide);
                }
            }
            '%' => tokens.push(Token::Modulo),
            '^' => tokens.push(Token::BitXor),
            '&' => tokens.push(Token::BitAnd),
            '|' => tokens.push(Token::BitOr),
            '~' => tokens.push(Token::BitNot),
            '<' => {
                if chars.peek() == Some(&'<') {
                    chars.next();
                    tokens.push(Token::LShift);
                } else {
                    return Err(EvaluationError::ParseError(
                        "Unexpected character: <".to_string(),
                    ));
                }
            }
            '>' => {
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::RShift);
                } else {
                    return Err(EvaluationError::ParseError(
                        "Unexpected character: >".to_string(),
                    ));
                }
            }
            '(' => tokens.push(Token::LeftParen),
            ')' => tokens.push(Token::RightParen),
            ',' => tokens.push(Token::Comma),

            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = ch.to_string();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        ident.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Identifier(ident));
            }

            _ => {
                return Err(EvaluationError::ParseError(format!(
                    "Unexpected character: {}",
                    ch
                )));
            }
        }
    }

    Ok(tokens)
}

fn tokenize_number(
    first: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<Token, EvaluationError> {
    let mut s = String::new();
    s.push(first);

    // hex: 0x...
    if first == '0' {
        if let Some(&c) = chars.peek() {
            if c == 'x' || c == 'X' {
                s.push(chars.next().unwrap());
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_hexdigit() || c == '_' {
                        if c != '_' {
                            s.push(c);
                        }
                        chars.next();
                    } else {
                        break;
                    }
                }
                let cleaned: String = s.chars().skip(2).collect();
                let val = u64::from_str_radix(&cleaned, 16).map_err(|_| {
                    EvaluationError::ParseError(format!("Invalid hex literal: {}", s))
                })?;
                return Ok(Token::Number(val as f64));
            }
            if c == 'o' || c == 'O' {
                s.push(chars.next().unwrap());
                while let Some(&c) = chars.peek() {
                    if ('0'..='7').contains(&c) || c == '_' {
                        if c != '_' {
                            s.push(c);
                        }
                        chars.next();
                    } else {
                        break;
                    }
                }
                let cleaned: String = s.chars().skip(2).collect();
                let val = u64::from_str_radix(&cleaned, 8).map_err(|_| {
                    EvaluationError::ParseError(format!("Invalid octal literal: {}", s))
                })?;
                return Ok(Token::Number(val as f64));
            }
            if c == 'b' || c == 'B' {
                s.push(chars.next().unwrap());
                while let Some(&c) = chars.peek() {
                    if c == '0' || c == '1' || c == '_' {
                        if c != '_' {
                            s.push(c);
                        }
                        chars.next();
                    } else {
                        break;
                    }
                }
                let cleaned: String = s.chars().skip(2).collect();
                let val = u64::from_str_radix(&cleaned, 2).map_err(|_| {
                    EvaluationError::ParseError(format!("Invalid binary literal: {}", s))
                })?;
                return Ok(Token::Number(val as f64));
            }
        }
    }

    // decimal digits and dots
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '_' {
            if c != '_' {
                s.push(c);
            }
            chars.next();
        } else {
            break;
        }
    }

    // decimal point
    if chars.peek() == Some(&'.') {
        // Make sure it's not followed by another dot (range operator etc.)
        s.push(chars.next().unwrap());
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() || c == '_' {
                if c != '_' {
                    s.push(c);
                }
                chars.next();
            } else {
                break;
            }
        }
    }

    // scientific notation: e/E +/- exponent
    if let Some(&c) = chars.peek() {
        if c == 'e' || c == 'E' {
            s.push(chars.next().unwrap());
            if let Some(&c) = chars.peek() {
                if c == '+' || c == '-' {
                    s.push(chars.next().unwrap());
                }
            }
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '_' {
                    if c != '_' {
                        s.push(c);
                    }
                    chars.next();
                } else {
                    break;
                }
            }
        }
    }

    let val: f64 = s
        .parse()
        .map_err(|_| EvaluationError::ParseError(format!("Invalid number: {}", s)))?;
    Ok(Token::Number(val))
}

// ─── Parser ──────────────────────────────────────────────────────────────────

fn parse_expression(expr: &str, depth: &mut usize) -> Result<f64, EvaluationError> {
    if *depth > MAX_NESTING_DEPTH {
        return Err(EvaluationError::StackOverflow);
    }
    let tokens = tokenize(expr)?;
    let mut pos = 0;
    let result = parse_bit_or(&tokens, &mut pos, depth)?;
    if pos < tokens.len() {
        return Err(EvaluationError::ParseError(format!(
            "Unexpected token after expression: {:?}",
            tokens[pos]
        )));
    }
    Ok(result)
}

// precedence 0 (lowest): |
fn parse_bit_or(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    let mut left = parse_bit_xor(tokens, pos, depth)?;
    while *pos < tokens.len() {
        if let Token::BitOr = &tokens[*pos] {
            *pos += 1;
            let right = parse_bit_xor(tokens, pos, depth)?;
            if left.fract() != 0.0 || right.fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "Bitwise operations require integer arguments".to_string(),
                ));
            }
            let l = left as i64;
            let r = right as i64;
            left = (l | r) as f64;
        } else {
            break;
        }
    }
    Ok(left)
}

// precedence 1: ^
fn parse_bit_xor(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    let mut left = parse_bit_and(tokens, pos, depth)?;
    while *pos < tokens.len() {
        if let Token::BitXor = &tokens[*pos] {
            *pos += 1;
            let right = parse_bit_and(tokens, pos, depth)?;
            if left.fract() != 0.0 || right.fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "Bitwise operations require integer arguments".to_string(),
                ));
            }
            let l = left as i64;
            let r = right as i64;
            left = (l ^ r) as f64;
        } else {
            break;
        }
    }
    Ok(left)
}

// precedence 2: &
fn parse_bit_and(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    let mut left = parse_shift(tokens, pos, depth)?;
    while *pos < tokens.len() {
        if let Token::BitAnd = &tokens[*pos] {
            *pos += 1;
            let right = parse_shift(tokens, pos, depth)?;
            if left.fract() != 0.0 || right.fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "Bitwise operations require integer arguments".to_string(),
                ));
            }
            let l = left as i64;
            let r = right as i64;
            left = (l & r) as f64;
        } else {
            break;
        }
    }
    Ok(left)
}

// precedence 3: << >>
fn parse_shift(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    let mut left = parse_additive(tokens, pos, depth)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::LShift => {
                *pos += 1;
                let right = parse_additive(tokens, pos, depth)?;
                let shift = right as i64;
                if shift < 0 || shift as usize > MAX_SHIFT_COUNT {
                    return Err(EvaluationError::InvalidOperation(format!(
                        "Shift count {} out of range",
                        shift
                    )));
                }
                let l = left as i64;
                left = l.checked_shl(shift as u32).ok_or_else(|| {
                    EvaluationError::InvalidOperation(format!("Shift left by {} overflows", shift))
                })? as f64;
            }
            Token::RShift => {
                *pos += 1;
                let right = parse_additive(tokens, pos, depth)?;
                let shift = right as i64;
                if shift < 0 || shift as usize > MAX_SHIFT_COUNT {
                    return Err(EvaluationError::InvalidOperation(format!(
                        "Shift count {} out of range",
                        shift
                    )));
                }
                let l = left as i64;
                left = l.checked_shr(shift as u32).ok_or_else(|| {
                    EvaluationError::InvalidOperation(format!("Shift right by {} overflows", shift))
                })? as f64;
            }
            _ => break,
        }
    }
    Ok(left)
}

// precedence 4: + -
fn parse_additive(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    let mut left = parse_multiplicative(tokens, pos, depth)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Plus => {
                *pos += 1;
                let right = parse_multiplicative(tokens, pos, depth)?;
                left = check_result_value(left + right)?;
            }
            Token::Minus => {
                *pos += 1;
                let right = parse_multiplicative(tokens, pos, depth)?;
                left = check_result_value(left - right)?;
            }
            _ => break,
        }
    }
    Ok(left)
}

// precedence 5: * / // %
fn parse_multiplicative(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    let mut left = parse_unary(tokens, pos, depth)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Multiply => {
                *pos += 1;
                let right = parse_unary(tokens, pos, depth)?;
                left = check_result_value(left * right)?;
            }
            Token::Divide => {
                *pos += 1;
                let right = parse_unary(tokens, pos, depth)?;
                if right == 0.0 {
                    return Err(EvaluationError::DivisionByZero);
                }
                left = check_result_value(left / right)?;
            }
            Token::FloorDiv => {
                *pos += 1;
                let right = parse_unary(tokens, pos, depth)?;
                if right == 0.0 {
                    return Err(EvaluationError::DivisionByZero);
                }
                left = check_result_value((left / right).floor())?;
            }
            Token::Modulo => {
                *pos += 1;
                let right = parse_unary(tokens, pos, depth)?;
                if right == 0.0 {
                    return Err(EvaluationError::DivisionByZero);
                }
                // Python-style floored modulo: result has same sign as divisor
                let mut result = left % right;
                if result != 0.0 && (result < 0.0) != (right < 0.0) {
                    result += right;
                }
                left = check_result_value(result)?;
            }
            _ => break,
        }
    }
    Ok(left)
}

// precedence 6: **  (RIGHT-associative, binds tighter than unary)
fn parse_power(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    let base = parse_primary(tokens, pos, depth)?;

    if *pos < tokens.len() {
        if let Token::Power = &tokens[*pos] {
            *pos += 1;
            // Exponent: parse_unary allows -100 in 10 ** -100
            // parse_unary → parse_power → parse_primary, so exponent
            // can itself be a power expression (right-associative)
            let exp = parse_unary(tokens, pos, depth)?;
            if exp.abs() > MAX_EXPONENT {
                return Err(EvaluationError::InvalidOperation(format!(
                    "Exponent {} out of range (max {})",
                    exp, MAX_EXPONENT
                )));
            }
            // Tolerance: round exponent if very close to integer
            let exp = if (exp - exp.round()).abs() < 1e-9 {
                exp.round()
            } else {
                exp
            };
            // Validate negative base with non-integer exponent
            if base < 0.0 && exp.fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "Cannot raise negative number to non-integer power".to_string(),
                ));
            }
            // Validate zero base with negative exponent
            if base == 0.0 && exp < 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "Cannot raise zero to a negative power".to_string(),
                ));
            }
            // Safe power: use integer arithmetic for large integer exponents
            if exp.fract() == 0.0 && base.fract() == 0.0 && exp.abs() < 1e15 && base.abs() < 1e15 {
                let base_i = base as i64;
                let exp_i = exp as i64;
                if exp_i >= 0 {
                    if let Some(result) = base_i.checked_pow(exp_i as u32) {
                        return Ok(result as f64);
                    }
                }
            }
            return check_result_value(base.powf(exp));
        }
    }

    Ok(base)
}

// precedence 7: unary + - ~  (binds less tightly than **)
fn parse_unary(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    if *pos >= tokens.len() {
        return parse_power(tokens, pos, depth);
    }
    match &tokens[*pos] {
        Token::Minus => {
            *pos += 1;
            let value = parse_power(tokens, pos, depth)?;
            Ok(-value)
        }
        Token::Plus => {
            *pos += 1;
            parse_power(tokens, pos, depth)
        }
        Token::BitNot => {
            *pos += 1;
            let value = parse_unary(tokens, pos, depth)?;
            if value.fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "Bitwise NOT requires integer argument".to_string(),
                ));
            }
            let i = value as i64;
            Ok((!i) as f64)
        }
        _ => parse_power(tokens, pos, depth),
    }
}

fn parse_primary(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    if *pos >= tokens.len() {
        return Err(EvaluationError::ParseError(
            "Unexpected end of expression".to_string(),
        ));
    }

    match &tokens[*pos] {
        Token::Number(n) => {
            *pos += 1;
            Ok(*n)
        }
        Token::Identifier(name) => {
            *pos += 1;
            parse_identifier(name, tokens, pos, depth)
        }
        Token::LeftParen => {
            *pos += 1;
            *depth += 1;
            if *depth > MAX_NESTING_DEPTH {
                return Err(EvaluationError::StackOverflow);
            }
            let value = parse_bit_or(tokens, pos, depth)?;
            *depth -= 1;
            match tokens.get(*pos) {
                Some(Token::RightParen) => {
                    *pos += 1;
                }
                _ => {
                    return Err(EvaluationError::ParseError(
                        "Missing closing parenthesis".into(),
                    ))
                }
            }
            Ok(value)
        }
        _ => Err(EvaluationError::ParseError("Unexpected token".to_string())),
    }
}

fn parse_identifier(
    name: &str,
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    if *pos < tokens.len() {
        if let Token::LeftParen = &tokens[*pos] {
            *pos += 1;
            return evaluate_function(name, tokens, pos, depth);
        }
    }

    // Check unit aliases first (case-sensitive), matching Python behavior.
    // "g" → 1 gram via UNIT_ALIASES, not gravity constant.
    if crate::calc::units::UNIT_ALIASES.get(name).is_some() {
        // Known unit — treat as unit reference (value 1.0, unit handled downstream)
        // But we need to return the numeric value; unit resolution happens in normalize.
        // For bare unit references in the evaluator, return 1.0.
        return Ok(1.0);
    }

    // Case-sensitive constant lookup
    if let Some(&value) = CONSTANTS.get(name) {
        return Ok(value);
    }

    // Case-insensitive fallback
    let lower = name.to_lowercase();
    if let Some(&value) = CONSTANTS.get(lower.as_str()) {
        return Ok(value);
    }

    Err(EvaluationError::UnknownConstant(name.to_string()))
}

// ─── Functions ───────────────────────────────────────────────────────────────

fn parse_args(
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<Vec<f64>, EvaluationError> {
    let mut args = Vec::new();
    let mut closed = false;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::RightParen => {
                *pos += 1;
                closed = true;
                break;
            }
            Token::Comma => {
                *pos += 1;
            }
            _ => {
                args.push(parse_bit_or(tokens, pos, depth)?);
            }
        }
    }
    if !closed {
        return Err(EvaluationError::InvalidOperation(
            "Missing closing parenthesis".to_string(),
        ));
    }
    Ok(args)
}

/// Banker's rounding (round half to even), matching Python's round().
fn banker_round(x: f64) -> f64 {
    let floor = x.floor();
    let frac = x - floor;
    if (frac - 0.5).abs() < f64::EPSILON {
        let even_floor = floor as i64;
        if even_floor % 2 == 0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        x.round()
    }
}

fn evaluate_function(
    name: &str,
    tokens: &[Token],
    pos: &mut usize,
    depth: &mut usize,
) -> Result<f64, EvaluationError> {
    let args = parse_args(tokens, pos, depth)?;
    let n = name.to_lowercase();

    // MCP-safe mode: reject random and side-effect functions
    check_function_allowed(name)?;

    match n.as_str() {
        // ── Trigonometric ──
        "sin" if args.len() == 1 => Ok(args[0].sin()),
        "cos" if args.len() == 1 => Ok(args[0].cos()),
        "tan" if args.len() == 1 => Ok(args[0].tan()),
        "asin" if args.len() == 1 => Ok(args[0].asin()),
        "acos" if args.len() == 1 => Ok(args[0].acos()),
        "atan" if args.len() == 1 => Ok(args[0].atan()),
        "atan2" if args.len() == 2 => Ok(args[0].atan2(args[1])),

        // ── Hyperbolic ──
        "sinh" if args.len() == 1 => Ok(args[0].sinh()),
        "cosh" if args.len() == 1 => Ok(args[0].cosh()),
        "tanh" if args.len() == 1 => Ok(args[0].tanh()),
        "asinh" if args.len() == 1 => Ok(args[0].asinh()),
        "acosh" if args.len() == 1 => Ok(args[0].acosh()),
        "atanh" if args.len() == 1 => Ok(args[0].atanh()),

        // ── Logarithmic / Exponential ──
        "log" | "ln" if args.len() == 1 => {
            if args[0] <= 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "logarithm of non-positive number".to_string(),
                ));
            }
            Ok(args[0].ln())
        }
        "log" | "ln" if args.len() == 2 => {
            // log(x, base) = ln(x) / ln(base)
            if args[0] <= 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "logarithm of non-positive number".to_string(),
                ));
            }
            if args[1] <= 0.0 || args[1] == 1.0 {
                return Err(EvaluationError::InvalidOperation(
                    "log base must be positive and not 1".to_string(),
                ));
            }
            Ok(args[0].ln() / args[1].ln())
        }
        "log10" if args.len() == 1 => {
            if args[0] <= 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "logarithm of non-positive number".to_string(),
                ));
            }
            Ok(args[0].log10())
        }
        "log2" if args.len() == 1 => {
            if args[0] <= 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "logarithm of non-positive number".to_string(),
                ));
            }
            Ok(args[0].log2())
        }
        "log1p" if args.len() == 1 => Ok(args[0].ln_1p()),
        "exp" if args.len() == 1 => {
            let result = args[0].exp();
            if result.is_infinite() || result.abs() > 1e308 {
                return Err(EvaluationError::InvalidOperation(
                    "Result too large".to_string(),
                ));
            }
            Ok(result)
        }
        "expm1" if args.len() == 1 => Ok(args[0].exp_m1()),

        // ── Power / Root ──
        "sqrt" if args.len() == 1 => {
            if args[0] < 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "square root of negative number".to_string(),
                ));
            }
            Ok(args[0].sqrt())
        }
        "cbrt" if args.len() == 1 => Ok(args[0].cbrt()),
        "pow" | "power" if args.len() == 2 => {
            if args[1].abs() > MAX_EXPONENT {
                return Err(EvaluationError::InvalidOperation(format!(
                    "Exponent {} out of range",
                    args[1]
                )));
            }
            if args[0] < 0.0 && args[1].fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "Cannot raise negative number to non-integer power".to_string(),
                ));
            }
            if args[0] == 0.0 && args[1] < 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "Cannot raise zero to a negative power".to_string(),
                ));
            }
            check_result_value(args[0].powf(args[1]))
        }

        // ── Rounding / Absolute ──
        "abs" if args.len() == 1 => Ok(args[0].abs()),
        "floor" if args.len() == 1 => Ok(args[0].floor()),
        "ceil" if args.len() == 1 => Ok(args[0].ceil()),
        "round" if args.len() == 1 => Ok(banker_round(args[0])),
        "round" if args.len() == 2 => {
            let ndigits = args[1] as i32;
            let factor = 10.0_f64.powi(ndigits);
            Ok(banker_round(args[0] * factor) / factor)
        }
        "trunc" if args.len() == 1 => Ok(args[0].trunc()),
        "sign" if args.len() == 1 => Ok(args[0].signum()),

        // ── Angle conversion ──
        "degrees" if args.len() == 1 => Ok(args[0].to_degrees()),
        "radians" if args.len() == 1 => Ok(args[0].to_radians()),

        // ── Hypotenuse ──
        "hypot" if !args.is_empty() => {
            // Use scale-based algorithm to avoid intermediate overflow
            // This matches Python's math.hypot implementation
            let max_val = args.iter().map(|x| x.abs()).fold(0.0, f64::max);
            if max_val == 0.0 {
                return Ok(0.0);
            }
            let sum_sq: f64 = args.iter().map(|x| (x / max_val).powi(2)).sum();
            Ok(max_val * sum_sq.sqrt())
        }

        // ── Factorial / Combinatorics ──
        "factorial" | "fact" if args.len() == 1 => {
            let n = args[0] as i64;
            if args[0] != n as f64 || n < 0 || n > MAX_FACTORIAL {
                return Err(EvaluationError::InvalidOperation(format!(
                    "factorial({}) out of range (0..={})",
                    args[0], MAX_FACTORIAL
                )));
            }
            // BUG-002 / parity B2: use big-integer arithmetic to avoid
            // f64 rounding for n > 170. Surface via __int_result__ so the
            // MCP layer reports type "int" instead of "float".
            let s = factorial_bigint(n as u64);
            return Err(EvaluationError::InvalidOperation(format!(
                "__int_result__{}",
                s
            )));
        }
        "perm" if args.len() == 1 => {
            let n = args[0] as i64;
            if args[0] != n as f64 || n < 0 || n > MAX_FACTORIAL {
                return Err(EvaluationError::InvalidOperation(format!(
                    "perm({}) out of range (0..={})",
                    args[0], MAX_FACTORIAL
                )));
            }
            let s = factorial_bigint(n as u64);
            return Err(EvaluationError::InvalidOperation(format!(
                "__int_result__{}",
                s
            )));
        }
        "perm" | "npr" if args.len() == 2 => {
            let n = args[0] as i64;
            let r = args[1] as i64;
            if args[0] != n as f64 || args[1] != r as f64 {
                return Err(EvaluationError::InvalidOperation(
                    "perm requires integer arguments".to_string(),
                ));
            }
            if n < 0 || r < 0 || n > MAX_PERM_COMB || r > MAX_PERM_COMB {
                return Err(EvaluationError::InvalidOperation(format!(
                    "perm({}, {}) out of range",
                    n, r
                )));
            }
            if r > n {
                return Ok(0.0);
            }
            let mut result = 1.0;
            for i in 0..r as u64 {
                result *= (n as u64 - i) as f64;
            }
            Ok(result)
        }
        "comb" | "ncr" if args.len() == 2 => {
            let n = args[0] as i64;
            let r = args[1] as i64;
            if args[0] != n as f64 || args[1] != r as f64 {
                return Err(EvaluationError::InvalidOperation(
                    "comb requires integer arguments".to_string(),
                ));
            }
            if n < 0 || r < 0 || n > MAX_PERM_COMB || r > MAX_PERM_COMB {
                return Err(EvaluationError::InvalidOperation(format!(
                    "comb({}, {}) out of range",
                    n, r
                )));
            }
            if r > n {
                return Ok(0.0);
            }
            let r_small = r.min(n - r);
            let mut result = 1.0;
            for i in 0..r_small as u64 {
                result *= (n as u64 - i) as f64;
                result /= (i + 1) as f64;
            }
            Ok(result)
        }

        // ── GCD / LCM (variadic) ──
        "gcd" if !args.is_empty() => {
            for a in &args {
                if *a != a.floor() || !a.is_finite() {
                    return Err(EvaluationError::InvalidOperation(
                        "gcd() requires integer arguments".to_string(),
                    ));
                }
            }
            let ints: Vec<i64> = args.iter().map(|a| *a as i64).collect();
            let mut result = ints[0].abs();
            for &v in &ints[1..] {
                result = gcd(result, v.abs());
            }
            Ok(result as f64)
        }
        "lcm" if !args.is_empty() => {
            for a in &args {
                if *a != a.floor() || !a.is_finite() {
                    return Err(EvaluationError::InvalidOperation(
                        "lcm() requires integer arguments".to_string(),
                    ));
                }
            }
            let ints: Vec<i64> = args.iter().map(|a| *a as i64).collect();
            let mut result = ints[0].abs();
            for &v in &ints[1..] {
                let g = gcd(result, v.abs());
                if g == 0 {
                    result = 0;
                } else {
                    result = (result / g)
                        .abs()
                        .checked_mul(v.abs())
                        .ok_or_else(|| EvaluationError::ValueOverflow)?;
                }
            }
            Ok(result as f64)
        }

        // ── Aggregate (variadic) ──
        "max" if !args.is_empty() => Ok(args.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
        "min" if !args.is_empty() => Ok(args.iter().cloned().fold(f64::INFINITY, f64::min)),
        "sum" if args.is_empty() => Ok(0.0),
        "sum" => {
            let s: f64 = args.iter().sum();
            check_result_value(s)
        }
        "mean" | "average" if !args.is_empty() => {
            let s: f64 = args.iter().sum();
            check_result_value(s / args.len() as f64)
        }
        "median" if !args.is_empty() => {
            if args.iter().any(|a| a.is_nan()) {
                return Err(EvaluationError::InvalidOperation(
                    "median does not support NaN values".to_string(),
                ));
            }
            let mut sorted = args.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = sorted.len() / 2;
            if sorted.len() % 2 == 0 {
                Ok((sorted[mid - 1] + sorted[mid]) / 2.0)
            } else {
                Ok(sorted[mid])
            }
        }
        "mode" if !args.is_empty() => {
            fn normalize_zero_bits(v: f64) -> u64 {
                if v == 0.0 {
                    0.0_f64.to_bits()
                } else {
                    v.to_bits()
                }
            }
            let mut counts: HashMap<u64, usize> = HashMap::new();
            for &a in &args {
                let key = normalize_zero_bits(a);
                *counts.entry(key).or_insert(0) += 1;
            }
            let max_count = counts.values().max().unwrap_or(&0);
            for &a in &args {
                if counts.get(&normalize_zero_bits(a)).unwrap_or(&0) == max_count {
                    return Ok(a);
                }
            }
            Ok(args[0])
        }
        "product" if !args.is_empty() => {
            let p: f64 = args.iter().fold(1.0, |acc, x| acc * x);
            check_result_value(p)
        }
        "std" | "stddev" if args.len() >= 2 => {
            let mean: f64 = args.iter().sum::<f64>() / args.len() as f64;
            let variance: f64 =
                args.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / args.len() as f64;
            Ok(variance.sqrt())
        }
        "std" | "stddev" if args.len() == 1 => Err(EvaluationError::InvalidOperation(
            "std requires at least two arguments".to_string(),
        )),
        "std_sample" | "stds" if args.len() >= 2 => {
            let mean: f64 = args.iter().sum::<f64>() / args.len() as f64;
            let variance: f64 =
                args.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (args.len() - 1) as f64;
            Ok(variance.sqrt())
        }
        "variance" | "var" if args.len() >= 2 => {
            let mean: f64 = args.iter().sum::<f64>() / args.len() as f64;
            let variance: f64 =
                args.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / args.len() as f64;
            Ok(variance)
        }
        "variance" | "var" if args.len() == 1 => Err(EvaluationError::InvalidOperation(
            "variance requires at least two arguments".to_string(),
        )),
        "variance_sample" | "vars" | "var_sample" if args.len() >= 2 => {
            let mean: f64 = args.iter().sum::<f64>() / args.len() as f64;
            let variance: f64 =
                args.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (args.len() - 1) as f64;
            Ok(variance)
        }

        // ── Percentage ──
        "percentof" | "percent_of" if args.len() == 2 => Ok(args[0] / 100.0 * args[1]),
        "aspercent" | "as_percent" if args.len() == 2 => {
            if args[1] == 0.0 {
                return Err(EvaluationError::DivisionByZero);
            }
            if args[1].abs() < 1e-100 {
                return Err(EvaluationError::InvalidOperation(
                    "aspercent: near-zero divisor could cause overflow".to_string(),
                ));
            }
            Ok(args[0] / args[1] * 100.0)
        }

        // ── Clamp ──
        "clamp" if args.len() == 3 => {
            if args[1] > args[2] {
                return Err(EvaluationError::InvalidOperation(format!(
                    "clamp: lo ({}) > hi ({})",
                    args[1], args[2]
                )));
            }
            Ok(args[0].max(args[1]).min(args[2]))
        }

        // ── Bitwise functions ──
        "bitand" if args.len() == 2 => {
            if args[0].fract() != 0.0 || args[1].fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "bitand requires integer arguments".to_string(),
                ));
            }
            Ok(((args[0] as i64) & (args[1] as i64)) as f64)
        }
        "bitor" if args.len() == 2 => {
            if args[0].fract() != 0.0 || args[1].fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "bitor requires integer arguments".to_string(),
                ));
            }
            Ok(((args[0] as i64) | (args[1] as i64)) as f64)
        }
        "bitxor" if args.len() == 2 => {
            if args[0].fract() != 0.0 || args[1].fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "bitxor requires integer arguments".to_string(),
                ));
            }
            Ok(((args[0] as i64) ^ (args[1] as i64)) as f64)
        }
        "bitnot" if args.len() == 1 => {
            if args[0].fract() != 0.0 {
                return Err(EvaluationError::InvalidOperation(
                    "bitnot requires integer argument".to_string(),
                ));
            }
            Ok((!(args[0] as i64)) as f64)
        }
        "bitlshift" if args.len() == 2 => {
            let shift = args[1] as i64;
            if shift < 0 || shift as usize > MAX_SHIFT_COUNT {
                return Err(EvaluationError::InvalidOperation(format!(
                    "Shift count {} out of range",
                    shift
                )));
            }
            Ok(((args[0] as i64).checked_shl(shift as u32).ok_or_else(|| {
                EvaluationError::InvalidOperation(format!("Shift left by {} overflows", shift))
            })?) as f64)
        }
        "bitrshift" if args.len() == 2 => {
            let shift = args[1] as i64;
            if shift < 0 || shift as usize > MAX_SHIFT_COUNT {
                return Err(EvaluationError::InvalidOperation(format!(
                    "Shift count {} out of range",
                    shift
                )));
            }
            Ok(((args[0] as i64).checked_shr(shift as u32).ok_or_else(|| {
                EvaluationError::InvalidOperation(format!("Shift right by {} overflows", shift))
            })?) as f64)
        }

        // ── Base conversion ──
        "bin" if args.len() == 1 => {
            let n = args[0] as i64;
            if args[0] != n as f64 {
                return Err(EvaluationError::InvalidOperation(
                    "bin requires integer argument".to_string(),
                ));
            }
            let s = if n < 0 {
                format!("-0b{:b}", n.wrapping_neg())
            } else {
                format!("0b{:b}", n)
            };
            Err(EvaluationError::InvalidOperation(format!(
                "__string_result__{}",
                s
            )))
        }
        "hex" if args.len() == 1 => {
            let n = args[0] as i64;
            if args[0] != n as f64 {
                return Err(EvaluationError::InvalidOperation(
                    "hex requires integer argument".to_string(),
                ));
            }
            let s = if n < 0 {
                format!("-0x{:x}", n.wrapping_neg())
            } else {
                format!("0x{:x}", n)
            };
            Err(EvaluationError::InvalidOperation(format!(
                "__string_result__{}",
                s
            )))
        }
        "oct" if args.len() == 1 => {
            let n = args[0] as i64;
            if args[0] != n as f64 {
                return Err(EvaluationError::InvalidOperation(
                    "oct requires integer argument".to_string(),
                ));
            }
            let s = if n < 0 {
                format!("-0o{:o}", n.wrapping_neg())
            } else {
                format!("0o{:o}", n)
            };
            return Err(EvaluationError::InvalidOperation(format!(
                "__string_result__{}",
                s
            )));
        }

        // ── Prime number functions ──
        "isprime" | "is_prime" if args.len() == 1 => {
            let n = args[0] as i64;
            if args[0] != n as f64 || n > MAX_PRIME {
                return Err(EvaluationError::InvalidOperation(format!(
                    "isprime({}) out of range",
                    args[0]
                )));
            }
            Ok(if n < 2 {
                0.0
            } else if is_prime(n) {
                1.0
            } else {
                0.0
            })
        }
        "nextprime" | "next_prime" if args.len() == 1 => {
            let n = args[0] as i64;
            if args[0] != n as f64 || n < 0 {
                return Err(EvaluationError::InvalidOperation(format!(
                    "nextprime({}) out of range",
                    args[0]
                )));
            }
            Ok(next_prime(n)? as f64)
        }
        "prevprime" | "prev_prime" if args.len() == 1 => {
            let n = args[0] as i64;
            if args[0] != n as f64 || n <= 2 {
                return Err(EvaluationError::InvalidOperation(format!(
                    "prevprime({}) out of range",
                    args[0]
                )));
            }
            Ok(prev_prime(n)? as f64)
        }
        "primefactors" | "prime_factors" if args.len() == 1 => {
            let n = args[0] as i64;
            if args[0] != n as f64 || n < 0 {
                return Err(EvaluationError::InvalidOperation(format!(
                    "primefactors({}) out of range",
                    args[0]
                )));
            }
            if n > 1_000_000_000_000 {
                return Err(EvaluationError::InvalidOperation(
                    "factorization not available for numbers > 10^12".to_string(),
                ));
            }
            let s = prime_factors_string(n);
            return Err(EvaluationError::InvalidOperation(format!(
                "__string_result__{}",
                s
            )));
        }

        // ── Complex number functions (f64 simplification) ──
        "real" if args.len() == 1 => Ok(args[0]),
        "imag" | "imaginary" if args.len() == 1 => Ok(0.0),
        "conj" | "conjugate" if args.len() == 1 => Ok(args[0]),
        "phase" | "arg" | "argument" if args.len() == 1 => Ok(if args[0] >= 0.0 {
            0.0
        } else {
            std::f64::consts::PI
        }),
        // BUG-003 / parity B3: Python's cmath.polar(z) takes a single
        // complex arg and returns (r, phi). When called with two real
        // args (the common calculator convention), treat them as
        // (r, phi) and return the same tuple.
        "polar" if args.len() == 1 => {
            let r = args[0].abs();
            let phi = if args[0] >= 0.0 {
                0.0
            } else {
                std::f64::consts::PI
            };
            let s = format!("({}, {})", r, phi);
            return Err(EvaluationError::InvalidOperation(format!(
                "__string_result__{}",
                s
            )));
        }
        "polar" if args.len() == 2 => {
            let s = format!("({}, {})", args[0], args[1]);
            return Err(EvaluationError::InvalidOperation(format!(
                "__string_result__{}",
                s
            )));
        }
        // BUG-004 / parity B4: Python's cmath.rect(r, phi) returns
        // r * (cos(phi) + sin(phi)j). Return a (real, imag) tuple string
        // instead of just echoing (r, phi).
        "rect" if args.len() == 2 => {
            let r = args[0];
            let phi = args[1];
            let real = r * phi.cos();
            let imag = r * phi.sin();
            let s = format!("({}, {})", real, imag);
            return Err(EvaluationError::InvalidOperation(format!(
                "__string_result__{}",
                s
            )));
        }

        // ── Random functions ──
        "random" | "rand" if args.is_empty() => Ok(prng_random()),
        "randint" if args.len() == 2 => {
            let a = args[0] as i64;
            let b = args[1] as i64;
            if args[0] != a as f64 || args[1] != b as f64 {
                return Err(EvaluationError::InvalidOperation(
                    "randint requires integer arguments".to_string(),
                ));
            }
            if a > b {
                return Err(EvaluationError::InvalidOperation(format!(
                    "randint: lower bound ({}) > upper bound ({})",
                    a, b
                )));
            }
            let range = (b - a + 1) as u64;
            let r = prng_random() * range as f64;
            Ok((a + r.floor() as i64) as f64)
        }
        "randrange" if args.len() == 1 => {
            let a = args[0] as i64;
            if args[0] != a as f64 {
                return Err(EvaluationError::InvalidOperation(
                    "randrange requires integer argument".to_string(),
                ));
            }
            if a <= 0 {
                return Err(EvaluationError::InvalidOperation(
                    "randrange: argument must be positive".to_string(),
                ));
            }
            let r = prng_random() * a as f64;
            Ok((r.floor() as i64) as f64)
        }
        "randrange" if args.len() == 2 => {
            let a = args[0] as i64;
            let b = args[1] as i64;
            if args[0] != a as f64 || args[1] != b as f64 {
                return Err(EvaluationError::InvalidOperation(
                    "randrange requires integer arguments".to_string(),
                ));
            }
            if a >= b {
                return Err(EvaluationError::InvalidOperation(
                    "randrange: start must be less than stop".to_string(),
                ));
            }
            let range = (b - a) as u64;
            let r = prng_random() * range as f64;
            Ok((a + r.floor() as i64) as f64)
        }
        "uniform" if args.len() == 2 => {
            let a = args[0];
            let b = args[1];
            if a > b {
                return Err(EvaluationError::InvalidOperation(format!(
                    "uniform: lower bound ({}) > upper bound ({})",
                    a, b
                )));
            }
            Ok(a + prng_random() * (b - a))
        }
        "randn" if args.is_empty() => Ok(prng_randn()),
        "gauss" | "normal" if args.len() == 2 => {
            let mu = args[0];
            let sigma = args[1];
            Ok(mu + sigma * prng_randn())
        }
        "seed" if args.is_empty() => {
            let mut state = PRNG_STATE.lock().unwrap();
            *state = 123456789;
            let mut spare = GAUSS_SPARE.lock().unwrap();
            *spare = None;
            Ok(0.0)
        }
        "seed" if args.len() == 1 => {
            let s = args[0] as u64;
            prng_seed(s);
            Ok(0.0)
        }

        // ── Memory / variable functions ──
        "store" if args.len() == 1 => {
            let mut regs = MEMORY_REGISTERS.lock().unwrap();
            regs.insert("M".to_string(), args[0]);
            Ok(args[0])
        }
        "store" if args.len() == 2 => {
            let name = format!("R{}", args[1] as i64);
            let mut regs = MEMORY_REGISTERS.lock().unwrap();
            regs.insert(name, args[0]);
            Ok(args[0])
        }
        "recall" if args.is_empty() => {
            let regs = MEMORY_REGISTERS.lock().unwrap();
            Ok(*regs.get("M").unwrap_or(&0.0))
        }
        "recall" if args.len() == 1 => {
            let name = format!("R{}", args[0] as i64);
            let regs = MEMORY_REGISTERS.lock().unwrap();
            Ok(*regs.get(&name).unwrap_or(&0.0))
        }
        "mplus" | "m+" | "madd" if args.len() == 1 => {
            let mut regs = MEMORY_REGISTERS.lock().unwrap();
            let current = *regs.get("M").unwrap_or(&0.0);
            let new_val = current + args[0];
            regs.insert("M".to_string(), new_val);
            Ok(new_val)
        }
        "mminus" | "m-" | "msub" if args.len() == 1 => {
            let mut regs = MEMORY_REGISTERS.lock().unwrap();
            let current = *regs.get("M").unwrap_or(&0.0);
            let new_val = current - args[0];
            regs.insert("M".to_string(), new_val);
            Ok(new_val)
        }
        "mc" | "mclear" if args.is_empty() => {
            let mut regs = MEMORY_REGISTERS.lock().unwrap();
            regs.clear();
            Ok(0.0)
        }
        "mr" | "mrecall" if args.is_empty() => {
            let regs = MEMORY_REGISTERS.lock().unwrap();
            Ok(*regs.get("M").unwrap_or(&0.0))
        }
        "setvar" if args.len() == 2 => {
            // args[0] is the value, args[1] is the variable name encoded as a number
            // Since we can't pass strings through f64, we use a numeric encoding
            // Actually, setvar in the evaluator receives (value, name_as_number)
            // The name is encoded by having the caller pass a numeric variable ID
            // For now, we store using a numeric key
            let var_id = args[1] as i64;
            let key = format!("v{}", var_id);
            let mut vars = USER_VARIABLES.lock().unwrap();
            if !vars.contains_key(&key) && vars.len() >= MAX_USER_VARIABLES {
                // Evict oldest entry
                if let Some(oldest) = vars.keys().next().cloned() {
                    vars.remove(&oldest);
                }
            }
            vars.insert(key, args[0]);
            Ok(args[0])
        }
        "getvar" if args.len() == 1 => {
            let var_id = args[0] as i64;
            let key = format!("v{}", var_id);
            let vars = USER_VARIABLES.lock().unwrap();
            Ok(*vars.get(&key).unwrap_or(&0.0))
        }
        "getvar" if args.len() == 2 => {
            let var_id = args[0] as i64;
            let key = format!("v{}", var_id);
            let vars = USER_VARIABLES.lock().unwrap();
            Ok(*vars.get(&key).unwrap_or(&args[1]))
        }
        "delvar" if args.len() == 1 => {
            let var_id = args[0] as i64;
            let key = format!("v{}", var_id);
            let mut vars = USER_VARIABLES.lock().unwrap();
            vars.remove(&key);
            Ok(0.0)
        }
        "listvars" if args.is_empty() => {
            let vars = USER_VARIABLES.lock().unwrap();
            let s = if vars.is_empty() {
                "{}".to_string()
            } else {
                let entries: Vec<String> =
                    vars.iter().map(|(k, v)| format!("{}: {}", k, v)).collect();
                format!("{{{}}}", entries.join(", "))
            };
            return Err(EvaluationError::InvalidOperation(format!(
                "__string_result__{}",
                s
            )));
        }
        "clearvars" if args.is_empty() => {
            let mut vars = USER_VARIABLES.lock().unwrap();
            vars.clear();
            Ok(0.0)
        }

        // ── Convert / Temp: handled in normalize.rs run(), error stub here ──
        "convert" => Err(EvaluationError::InvalidOperation(
            "convert() must be called through the run() pipeline, not evaluate() directly"
                .to_string(),
        )),
        "temp" => Err(EvaluationError::InvalidOperation(
            "temp() must be called through the run() pipeline, not evaluate() directly".to_string(),
        )),

        _ => Err(EvaluationError::UnknownFunction(format!(
            "{}({} args)",
            name,
            args.len()
        ))),
    }
}

// ─── Helper functions ────────────────────────────────────────────────────────

/// BUG-002 / parity B2: big-integer factorial. Returns the result as a
/// decimal string (no f64 rounding) so values up to MAX_FACTORIAL don't
/// lose precision. Implemented with base-1e9 chunks to keep multiplication
/// fast on large inputs.
fn factorial_bigint(n: u64) -> String {
    // BUG-002 / parity B2: implement factorial as base-1e9 little-endian
    // big integer multiplication. Lets us return exact 309+ digit results
    // for n up to MAX_FACTORIAL without f64 rounding.
    let mut limbs: Vec<u64> = vec![1];
    for i in 2..=n {
        multiply_in_place(&mut limbs, i);
    }
    let mut out = limbs.last().copied().unwrap_or(0).to_string();
    for &limb in limbs.iter().rev().skip(1) {
        out.push_str(&format!("{:09}", limb));
    }
    if out.is_empty() {
        out = "0".to_string();
    }
    out
}

/// Multiply a base-1e9 little-endian number by a small integer (<= BASE).
/// Uses 128-bit intermediates to avoid overflow during the carry step.
fn multiply_in_place(limbs: &mut Vec<u64>, multiplier: u64) {
    let mut carry: u64 = 0;
    for limb in limbs.iter_mut() {
        let product = (*limb as u128) * (multiplier as u128) + (carry as u128);
        *limb = (product % 1_000_000_000u128) as u64;
        carry = (product / 1_000_000_000u128) as u64;
    }
    while carry > 0 {
        limbs.push(carry % 1_000_000_000);
        carry /= 1_000_000_000;
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if a == 0 {
        return b.abs();
    }
    if b == 0 {
        return a.abs();
    }
    gcd(b, a % b)
}

fn is_prime(n: i64) -> bool {
    if n < 2 {
        return false;
    }
    if n < 4 {
        return true;
    }
    if n % 2 == 0 || n % 3 == 0 {
        return false;
    }
    let mut i = 5;
    while i * i <= n {
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
        i += 6;
    }
    true
}

fn next_prime(n: i64) -> Result<i64, EvaluationError> {
    let mut candidate = if n < 2 {
        2
    } else {
        n.checked_add(1)
            .ok_or_else(|| EvaluationError::ValueOverflow)?
    };
    if candidate % 2 == 0 && candidate > 2 {
        candidate = candidate
            .checked_add(1)
            .ok_or_else(|| EvaluationError::ValueOverflow)?;
    }
    let mut iterations = 0;
    while iterations < 10_000 {
        if is_prime(candidate) {
            return Ok(candidate);
        }
        candidate = candidate
            .checked_add(2)
            .ok_or_else(|| EvaluationError::ValueOverflow)?;
        iterations += 1;
    }
    Err(EvaluationError::InvalidOperation(
        "Could not find next prime within search limit".to_string(),
    ))
}

fn prev_prime(n: i64) -> Result<i64, EvaluationError> {
    if n <= 2 {
        return Err(EvaluationError::InvalidOperation(
            "prevprime: no prime less than 2 exists".to_string(),
        ));
    }
    let mut candidate = n - 1;
    if candidate % 2 == 0 {
        candidate -= 1;
    }
    let mut iterations = 0;
    while iterations < 10_000 {
        if is_prime(candidate) {
            return Ok(candidate);
        }
        candidate -= 2;
        if candidate < 2 {
            return Ok(2);
        }
        iterations += 1;
    }
    Err(EvaluationError::InvalidOperation(
        "Could not find previous prime within search limit".to_string(),
    ))
}

fn prime_factors_string(n: i64) -> String {
    if n < 2 {
        return n.to_string();
    }
    let mut factors: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut temp = n;
    let mut d = 2i64;
    while d * d <= temp {
        while temp % d == 0 {
            *factors.entry(d).or_insert(0) += 1;
            temp /= d;
        }
        d += 1;
    }
    if temp > 1 {
        *factors.entry(temp).or_insert(0) += 1;
    }
    let mut parts: Vec<String> = Vec::new();
    let mut primes: Vec<i64> = factors.keys().copied().collect();
    primes.sort();
    for prime in primes {
        let exp = factors[&prime];
        if exp == 1 {
            parts.push(prime.to_string());
        } else {
            parts.push(format!("{}^{}", prime, exp));
        }
    }
    parts.join(" × ")
}

// ─── PRNG helper (xorshift64) ────────────────────────────────────────────────

fn xorshift64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn prng_random() -> f64 {
    let mut state = PRNG_STATE.lock().unwrap();
    xorshift64(&mut state) as f64 / u64::MAX as f64
}

fn prng_seed(s: u64) {
    let mut state = PRNG_STATE.lock().unwrap();
    *state = if s == 0 { 123456789 } else { s };
    let mut spare = GAUSS_SPARE.lock().unwrap();
    *spare = None;
}

fn prng_randn() -> f64 {
    // Box-Muller transform
    let mut spare = GAUSS_SPARE.lock().unwrap();
    if let Some(val) = spare.take() {
        return val;
    }
    let mut state = PRNG_STATE.lock().unwrap();
    let u1 = (xorshift64(&mut state) as f64 / u64::MAX as f64).max(1e-300);
    let u2 = xorshift64(&mut state) as f64 / u64::MAX as f64;
    let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    let z1 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).sin();
    drop(state);
    *spare = Some(z1);
    z0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(expr: &str) -> String {
        evaluate(expr).unwrap().0
    }

    fn typ(expr: &str) -> String {
        evaluate(expr).unwrap().1
    }

    // ── Phase 1: Number parsing ──

    #[test]
    fn test_basic_arithmetic() {
        assert_eq!(val("5 + 3"), "8");
        assert_eq!(val("10 - 4"), "6");
        assert_eq!(val("6 * 7"), "42");
        assert_eq!(val("15 / 3"), "5");
        assert_eq!(val("17 % 5"), "2");
    }

    #[test]
    fn test_scientific_notation() {
        assert_eq!(val("1e5"), "100000");
        assert_eq!(val("1.5E-3"), "0.0015");
        assert_eq!(val(".5e+2"), "50");
        assert_eq!(val("1E3"), "1000");
    }

    #[test]
    fn test_hex_literal() {
        assert_eq!(val("0xFF"), "255");
        assert_eq!(val("0x0"), "0");
        assert_eq!(val("0xDEAD"), "57005");
        assert_eq!(val("0xff"), "255");
    }

    #[test]
    fn test_octal_literal() {
        assert_eq!(val("0o17"), "15");
        assert_eq!(val("0o77"), "63");
        assert_eq!(val("0o0"), "0");
    }

    #[test]
    fn test_binary_literal() {
        assert_eq!(val("0b1010"), "10");
        assert_eq!(val("0b0"), "0");
        assert_eq!(val("0b11111111"), "255");
    }

    #[test]
    fn test_hex_with_underscores() {
        assert_eq!(val("0xFF_FF"), "65535");
    }

    #[test]
    fn test_decimal_with_underscores() {
        assert_eq!(val("1_000"), "1000");
        assert_eq!(val("1_000_000"), "1000000");
    }

    #[test]
    fn test_power_right_associative() {
        assert_eq!(val("2 ** 3 ** 2"), "512");
        assert_eq!(val("2 ** 2 ** 3"), "256");
        assert_eq!(val("2 ** 3 ** 2 ** 1"), "512");
    }

    // ── Phase 2: Operators ──

    #[test]
    fn test_floor_division() {
        assert_eq!(val("7 // 2"), "3");
        assert_eq!(val("-7 // 2"), "-4");
        assert_eq!(val("7 // -2"), "-4");
        assert_eq!(val("10 // 3"), "3");
        assert_eq!(val("0 // 5"), "0");
    }

    #[test]
    fn test_bitwise_not() {
        assert_eq!(val("~0"), "-1");
        assert_eq!(val("~(-1)"), "0");
        assert_eq!(val("~255"), "-256");
    }

    #[test]
    fn test_bitwise_and() {
        assert_eq!(val("5 & 3"), "1");
        assert_eq!(val("0xFF & 0x0F"), "15");
        assert_eq!(val("12 & 10"), "8");
    }

    #[test]
    fn test_bitwise_or() {
        assert_eq!(val("5 | 3"), "7");
        assert_eq!(val("0 | 0xFF"), "255");
    }

    #[test]
    fn test_bitwise_xor() {
        assert_eq!(val("5 ^ 3"), "6");
        assert_eq!(val("0xFF ^ 0x0F"), "240");
    }

    #[test]
    fn test_shift_left() {
        assert_eq!(val("1 << 4"), "16");
        assert_eq!(val("5 << 3"), "40");
        assert_eq!(val("1 << 0"), "1");
    }

    #[test]
    fn test_shift_right() {
        assert_eq!(val("16 >> 2"), "4");
        assert_eq!(val("100 >> 3"), "12");
        assert_eq!(val("1 >> 0"), "1");
    }

    #[test]
    fn test_bitwise_precedence() {
        // | has lower precedence than ^
        assert_eq!(val("1 | 2 ^ 3"), "1");
        // ^ has lower precedence than &
        assert_eq!(val("6 ^ 3 & 1"), "7");
        // & has lower precedence than <<
        assert_eq!(val("1 << 4 & 0xFF"), "16");
    }

    #[test]
    fn test_unary_minus_with_power() {
        assert_eq!(val("-2 ** 2"), "-4");
        assert_eq!(val("-(2 ** 2)"), "-4");
    }

    // ── Phase 3: Functions ──

    #[test]
    fn test_inverse_hyperbolic() {
        let v: f64 = val("asinh(1)").parse().unwrap();
        assert!((v - 0.881373587019543).abs() < 1e-10);
        let v: f64 = val("acosh(1)").parse().unwrap();
        assert!(v.abs() < 1e-10);
        let v: f64 = val("atanh(0.5)").parse().unwrap();
        assert!((v - 0.5493061443340549).abs() < 1e-10);
    }

    #[test]
    fn test_log_with_base() {
        assert_eq!(val("log(100, 10)"), "2");
        assert_eq!(val("log(8, 2)"), "3");
        assert_eq!(val("ln(e)"), "1");
    }

    #[test]
    fn test_log1p_expm1() {
        let v: f64 = val("log1p(0)").parse().unwrap();
        assert!(v.abs() < 1e-10);
        let v: f64 = val("log1p(1)").parse().unwrap();
        assert!((v - std::f64::consts::LN_2).abs() < 1e-10);
        let v: f64 = val("expm1(0)").parse().unwrap();
        assert!(v.abs() < 1e-10);
        let v: f64 = val("expm1(1)").parse().unwrap();
        assert!((v - 1.718281828459045).abs() < 1e-10);
    }

    #[test]
    fn test_degrees_radians() {
        assert_eq!(val("degrees(pi)"), "180");
        assert_eq!(val("radians(180)"), "3.141592653589793");
        assert_eq!(val("degrees(0)"), "0");
        assert_eq!(val("radians(0)"), "0");
    }

    #[test]
    fn test_hypot() {
        let v: f64 = val("hypot(3, 4)").parse().unwrap();
        assert!((v - 5.0).abs() < 1e-10);
        assert_eq!(val("hypot(0, 0)"), "0");
    }

    #[test]
    fn test_perm_comb() {
        assert_eq!(val("perm(5, 2)"), "20");
        assert_eq!(val("comb(5, 2)"), "10");
        assert_eq!(val("nPr(5, 2)"), "20");
        assert_eq!(val("nCr(5, 2)"), "10");
        assert_eq!(val("perm(5)"), "120");
        // perm returns 0 when r > n (matching Python math.perm)
        assert_eq!(val("perm(3, 5)"), "0");
    }

    #[test]
    fn test_clamp() {
        assert_eq!(val("clamp(5, 0, 10)"), "5");
        assert_eq!(val("clamp(-5, 0, 10)"), "0");
        assert_eq!(val("clamp(15, 0, 10)"), "10");
    }

    #[test]
    fn test_mode() {
        assert_eq!(val("mode(1, 2, 2, 3)"), "2");
        assert_eq!(val("mode(1, 1, 2)"), "1");
        assert_eq!(val("mode(5)"), "5");
    }

    #[test]
    fn test_sample_stats() {
        let v: f64 = val("std_sample(2, 4, 4, 4, 5, 5, 7, 9)").parse().unwrap();
        assert!((v - 2.1381).abs() < 0.01);
        let v: f64 = val("variance_sample(2, 4, 4, 4, 5, 5, 7, 9)")
            .parse()
            .unwrap();
        assert!((v - 4.5714).abs() < 0.01);
        assert_eq!(val("stds(1, 1)"), "0");
        assert_eq!(val("vars(1, 1)"), "0");
    }

    #[test]
    fn test_is_prime() {
        assert_eq!(val("isprime(2)"), "1");
        assert_eq!(val("isprime(3)"), "1");
        assert_eq!(val("isprime(4)"), "0");
        assert_eq!(val("isprime(17)"), "1");
        assert_eq!(val("isprime(1)"), "0");
        assert_eq!(val("isprime(0)"), "0");
        assert_eq!(val("is_prime(97)"), "1");
    }

    #[test]
    fn test_next_prev_prime() {
        assert_eq!(val("nextprime(10)"), "11");
        assert_eq!(val("nextprime(11)"), "13");
        assert_eq!(val("prevprime(10)"), "7");
        assert_eq!(val("prevprime(11)"), "7");
        assert_eq!(val("next_prime(0)"), "2");
        assert_eq!(val("prev_prime(5)"), "3");
        // prevprime(2) should error — no prime less than 2
        assert!(evaluate("prevprime(2)").is_err());
        assert!(evaluate("prevprime(1)").is_err());
    }

    #[test]
    fn test_prime_factors() {
        assert_eq!(val("primefactors(12)"), "2^2 × 3");
        assert_eq!(val("prime_factors(12)"), "2^2 × 3");
        assert_eq!(val("primefactors(7)"), "7");
        assert_eq!(val("primefactors(100)"), "2^2 × 5^2");
        assert_eq!(val("primefactors(1)"), "1");
        assert_eq!(val("primefactors(0)"), "0");
        assert_eq!(val("primefactors(2)"), "2");
        assert_eq!(val("primefactors(36)"), "2^2 × 3^2");
        // Out of range
        assert!(evaluate("primefactors(-1)").is_err());
        assert!(evaluate("primefactors(1000000000001)").is_err());
    }

    #[test]
    fn test_percent() {
        assert_eq!(val("percentof(50, 200)"), "100");
        assert_eq!(val("percentof(25, 100)"), "25");
        assert_eq!(val("aspercent(25, 100)"), "25");
        assert_eq!(val("aspercent(1, 4)"), "25");
        assert_eq!(val("percent_of(50, 200)"), "100");
        assert_eq!(val("as_percent(1, 4)"), "25");
    }

    // ── Phase 4: Aliases and variadic ──

    #[test]
    fn test_factorial_alias() {
        assert_eq!(val("fact(5)"), "120");
        assert_eq!(val("fact(0)"), "1");
    }

    #[test]
    fn test_average_alias() {
        assert_eq!(val("average(2, 4, 6)"), "4");
    }

    #[test]
    fn test_variadic_gcd() {
        assert_eq!(val("gcd(12, 8, 4)"), "4");
        assert_eq!(val("gcd(7)"), "7");
    }

    #[test]
    fn test_variadic_lcm() {
        assert_eq!(val("lcm(4, 6)"), "12");
        assert_eq!(val("lcm(2, 3, 4)"), "12");
    }

    #[test]
    fn test_single_arg_std_var() {
        let r = evaluate("std(5)");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("requires at least two"));
        let r = evaluate("variance(5)");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("requires at least two"));
    }

    // ── Phase 5: Safety limits ──

    #[test]
    fn test_max_exponent_enforced() {
        let r = evaluate("2 ** 100001");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("out of range"));
    }

    #[test]
    fn test_factorial_range() {
        // BUG-001/B2: MAX_FACTORIAL raised from 170 to 1000, and the
        // factorial implementation now uses big-integer arithmetic so
        // values up to MAX_FACTORIAL succeed.
        let r = evaluate("factorial(1001)");
        assert!(r.is_err());
        assert_eq!(val("factorial(170)").len() > 0, true);
        assert_eq!(val("factorial(1000)").len() > 0, true);
    }

    #[test]
    fn test_shift_count_limit() {
        let r = evaluate("1 << 50001");
        assert!(r.is_err());
    }

    #[test]
    fn test_nesting_depth() {
        let mut deep = String::new();
        for _ in 0..101 {
            deep.push('(');
        }
        deep.push_str("1");
        for _ in 0..101 {
            deep.push(')');
        }
        let r = evaluate(&deep);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("Stack overflow"));
    }

    #[test]
    fn test_input_length_limit() {
        let long: String = "1 + ".repeat(3000);
        let r = evaluate(&long);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("exceeds"));
    }

    // ── Edge cases ──

    #[test]
    fn test_zero_power_zero() {
        assert_eq!(val("0 ** 0"), "1");
    }

    #[test]
    fn test_negative_base_integer_exponent() {
        assert_eq!(val("(-2) ** 3"), "-8");
        assert_eq!(val("(-2) ** 2"), "4");
    }

    #[test]
    fn test_complex_precedence() {
        assert_eq!(val("2 + 3 * 4"), "14");
        assert_eq!(val("(2 + 3) * 4"), "20");
        assert_eq!(val("2 * 3 + 4 * 5"), "26");
        assert_eq!(val("2 ** 3 * 2"), "16");
        assert_eq!(val("2 * 3 ** 2"), "18");
    }

    #[test]
    fn test_unknown_function_error() {
        let r = evaluate("foobar(1)");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("Unknown function"));
    }

    #[test]
    fn test_unknown_constant_error() {
        let r = evaluate("xyz");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("Unknown constant"));
    }

    #[test]
    fn test_division_by_zero() {
        let r = evaluate("1 / 0");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("Division by zero"));
    }

    #[test]
    fn test_floor_div_by_zero() {
        let r = evaluate("1 // 0");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("Division by zero"));
    }

    // ── MCP-safe mode tests ──

    #[test]
    fn test_set_mcp_mode_is_idempotent() {
        // set_mcp_mode() is safe to call multiple times
        set_mcp_mode();
        set_mcp_mode();
        assert!(is_mcp_mode());
    }

    #[test]
    fn test_is_mcp_mode_after_set() {
        // Once set, is_mcp_mode() returns true
        set_mcp_mode();
        assert!(is_mcp_mode());
    }

    #[test]
    fn test_mcp_mode_rejects_random_functions() {
        set_mcp_mode();
        let r = evaluate("random(1)");
        assert!(r.is_err());
        let err = r.unwrap_err();
        assert!(
            err.contains("non-deterministic") || err.contains("disabled"),
            "Expected random function rejection, got: {}",
            err
        );
    }

    #[test]
    fn test_mcp_mode_rejects_randint() {
        set_mcp_mode();
        let r = evaluate("randint(1, 10)");
        assert!(r.is_err());
        let err = r.unwrap_err();
        assert!(
            err.contains("non-deterministic") || err.contains("disabled"),
            "Expected randint rejection, got: {}",
            err
        );
    }

    #[test]
    fn test_mcp_mode_rejects_gauss() {
        set_mcp_mode();
        let r = evaluate("gauss(0, 1)");
        assert!(r.is_err());
        let err = r.unwrap_err();
        assert!(
            err.contains("non-deterministic") || err.contains("disabled"),
            "Expected gauss rejection, got: {}",
            err
        );
    }

    #[test]
    fn test_mcp_mode_rejects_side_effect_functions() {
        set_mcp_mode();
        // setvar with numeric args (Rust evaluator doesn't support string literals)
        let r = evaluate("setvar(1, 2)");
        assert!(r.is_err());
        let err = r.unwrap_err();
        assert!(
            err.contains("mutates") || err.contains("disabled"),
            "Expected side-effect function rejection, got: {}",
            err
        );
    }

    #[test]
    fn test_mcp_mode_allows_deterministic_functions() {
        set_mcp_mode();
        // Deterministic functions should still work
        assert_eq!(val("sin(0)"), "0");
        assert_eq!(val("sqrt(144)"), "12");
        assert_eq!(val("abs(-5)"), "5");
        assert_eq!(val("5 + 3"), "8");
        assert_eq!(val("factorial(5)"), "120");
    }

    #[test]
    fn test_mode_uniform_frequency() {
        assert_eq!(val("mode(1, 2, 3)"), "1");
    }

    #[test]
    fn test_mode_negative_zero() {
        assert_eq!(val("mode(0, -0, 1, 2)"), "0");
    }

    #[test]
    fn test_exp_overflow() {
        let r = evaluate("exp(1000)");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("too large"));
    }

    #[test]
    fn test_isprime_negative() {
        assert_eq!(val("isprime(-5)"), "0");
    }

    #[test]
    fn test_isprime_zero() {
        assert_eq!(val("isprime(0)"), "0");
    }

    #[test]
    fn test_isprime_one() {
        assert_eq!(val("isprime(1)"), "0");
    }

    #[test]
    fn test_sum_no_args() {
        assert_eq!(val("sum()"), "0");
    }

    #[test]
    fn test_negative_base_near_integer_exponent() {
        assert_eq!(val("(-2) ** 2.0000000001"), "4");
    }
}
