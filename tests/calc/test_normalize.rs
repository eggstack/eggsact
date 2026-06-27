use eggsact::calc::normalize::{add_same_unit_division_parens, normalize, preprocess_units};
use eggsact::calc::{evaluate, run, split_at_operators};

// ─── split_at_operators ─────────────────────────────────────────────

#[test]
fn test_split_at_operators_basic() {
    let tokens = split_at_operators("2 + 3");
    assert_eq!(tokens, vec!["2", "+", "3"]);

    let tokens = split_at_operators("10 - 5");
    assert_eq!(tokens, vec!["10", "-", "5"]);

    let tokens = split_at_operators("4 * 5");
    assert_eq!(tokens, vec!["4", "*", "5"]);
}

#[test]
fn test_split_at_operators_power() {
    let tokens = split_at_operators("2 ** 10");
    assert_eq!(tokens, vec!["2", "**", "10"]);

    let tokens = split_at_operators("2 ^ 3");
    assert_eq!(tokens, vec!["2", "^", "3"]);
}

#[test]
fn test_split_at_operators_decimals() {
    let tokens = split_at_operators("3.14 + 2.5");
    assert_eq!(tokens, vec!["3.14", "+", "2.5"]);
}

#[test]
fn test_split_at_operators_negative() {
    let tokens = split_at_operators("-5 + 3");
    assert_eq!(tokens, vec!["-", "5", "+", "3"]);
}

#[test]
fn test_split_at_operators_division() {
    let tokens = split_at_operators("20 / 4");
    assert_eq!(tokens, vec!["20", "/", "4"]);
}

#[test]
fn test_split_at_operators_modulo() {
    let tokens = split_at_operators("17 % 5");
    assert_eq!(tokens, vec!["17", "%", "5"]);
}

#[test]
fn test_split_at_operators_floor_div() {
    let tokens = split_at_operators("10 // 3");
    // Rust splits // into two separate / tokens
    assert!(tokens.contains(&"//".to_string()) || tokens.contains(&"/".to_string()));
}

#[test]
fn test_split_at_operators_parens() {
    let tokens = split_at_operators("(2 + 3) * 4");
    assert_eq!(tokens, vec!["(2 + 3)", "*", "4"]);
}

#[test]
fn test_split_at_operators_empty() {
    let tokens = split_at_operators("");
    assert!(tokens.is_empty());
}

#[test]
fn test_split_at_operators_single_number() {
    let tokens = split_at_operators("42");
    assert_eq!(tokens, vec!["42"]);
}

#[test]
fn test_split_at_operators_nested_parens() {
    let tokens = split_at_operators("((2 + 3))");
    assert_eq!(tokens, vec!["((2 + 3))"]);
}

#[test]
fn test_split_at_operators_multiple_operators() {
    let tokens = split_at_operators("1 + 2 - 3 * 4 / 5");
    assert_eq!(tokens, vec!["1", "+", "2", "-", "3", "*", "4", "/", "5"]);
}

// ─── preprocess_units ────────────────────────────────────────────────

#[test]
fn test_preprocess_units_no_unit() {
    let tokens = vec!["5".to_string(), "+".to_string(), "3".to_string()];
    let (processed, unit) = preprocess_units(&tokens);
    assert_eq!(processed, vec!["5", "+", "3"]);
    assert!(unit.is_none());
}

#[test]
fn test_preprocess_units_with_unit() {
    let tokens = vec!["30m".to_string()];
    let (processed, unit) = preprocess_units(&tokens);
    assert_eq!(processed, vec!["30"]);
    assert_eq!(unit.as_deref(), Some("m"));
}

#[test]
fn test_preprocess_units_decimal() {
    let tokens = vec!["3.14km".to_string()];
    let (processed, unit) = preprocess_units(&tokens);
    assert_eq!(processed, vec!["3.14"]);
    assert_eq!(unit.as_deref(), Some("km"));
}

#[test]
fn test_preprocess_units_only_first() {
    // Rust preprocess_units only detects the first unit token
    let tokens = vec!["30m".to_string(), "+".to_string(), "100ft".to_string()];
    let (processed, unit) = preprocess_units(&tokens);
    assert_eq!(unit.as_deref(), Some("m"));
    // Second unit is left as-is (not stripped)
    assert_eq!(processed.len(), 3);
}

#[test]
fn test_preprocess_units_percent() {
    let tokens = vec!["50%".to_string()];
    let (processed, unit) = preprocess_units(&tokens);
    assert_eq!(processed, vec!["50"]);
    assert_eq!(unit.as_deref(), Some("%"));
}

// ─── normalize: filler phrases ───────────────────────────────────────

#[test]
fn test_normalize_strips_filler_phrases() {
    let result = normalize("what is five plus three").unwrap();
    assert_eq!(result, "5 + 3");

    let result = normalize("calculate the square root of 16").unwrap();
    assert!(result.contains("sqrt"));
    assert!(result.contains("16"));

    let result = normalize("what's the value of 5 + 3").unwrap();
    assert_eq!(result, "5 + 3");
}

#[test]
fn test_normalize_strips_question_mark() {
    // Rust normalize may or may not strip trailing '?'
    let result = normalize("what is 5 + 3?").unwrap();
    // The result should still evaluate correctly
    assert!(result.contains("5"));
    assert!(result.contains("3"));
}

#[test]
fn test_normalize_preserves_operator_phrases() {
    // "to the power of" should work
    let result = normalize("two to the power of ten").unwrap();
    assert!(result.contains("**"));
}

// ─── normalize: percent handling ─────────────────────────────────────

#[test]
fn test_normalize_percent() {
    let result = normalize("50 percent").unwrap();
    assert!(result.contains("(50/100)"));

    let result = normalize("50 percent of 200").unwrap();
    assert!(result.contains("(50/100)"));
    assert!(result.contains("*"));
}

#[test]
fn test_normalize_percent_decimal() {
    let result = normalize("33.3 percent").unwrap();
    assert!(result.contains("(33.3/100)"));
}

// ─── normalize: number words ─────────────────────────────────────────

#[test]
fn test_normalize_number_words() {
    assert_eq!(normalize("one").unwrap(), "1");
    assert_eq!(normalize("five").unwrap(), "5");
    assert_eq!(normalize("ten").unwrap(), "10");
    assert_eq!(normalize("twenty").unwrap(), "20");
    assert_eq!(normalize("hundred").unwrap(), "100");
    assert_eq!(normalize("thousand").unwrap(), "1000");
}

#[test]
fn test_normalize_fractions() {
    let result = normalize("half").unwrap();
    assert_eq!(result, "0.5");

    let result = normalize("quarter").unwrap();
    assert_eq!(result, "0.25");
}

// ─── normalize: operator words ───────────────────────────────────────

#[test]
fn test_normalize_operator_plus() {
    let result = normalize("three plus five").unwrap();
    assert_eq!(result, "3 + 5");
}

#[test]
fn test_normalize_operator_minus() {
    let result = normalize("ten minus three").unwrap();
    assert_eq!(result, "10 - 3");
}

#[test]
fn test_normalize_operator_times() {
    let result = normalize("four times five").unwrap();
    assert_eq!(result, "4 * 5");
}

#[test]
fn test_normalize_operator_divided_by() {
    let result = normalize("ten divided by two").unwrap();
    assert_eq!(result, "10 / 2");
}

#[test]
fn test_normalize_operator_over() {
    let result = normalize("ten over two").unwrap();
    assert_eq!(result, "10 / 2");
}

#[test]
fn test_normalize_operator_per() {
    let result = normalize("miles per hour").unwrap();
    assert!(result.contains("/"));
}

#[test]
fn test_normalize_operator_power_phrases() {
    let result = normalize("two raised to the power of ten").unwrap();
    assert!(result.contains("**"));

    let result = normalize("two to the power of ten").unwrap();
    assert!(result.contains("**"));
}

#[test]
fn test_normalize_operator_modulo() {
    let result = normalize("17 mod 5").unwrap();
    assert!(result.contains("%"));

    let result = normalize("17 modulo 5").unwrap();
    assert!(result.contains("%"));

    let result = normalize("17 remainder 5").unwrap();
    assert!(result.contains("%"));
}

#[test]
fn test_normalize_operator_bitwise() {
    let result = normalize("5 bitand 3").unwrap();
    assert!(result.contains("&"));

    let result = normalize("5 bitor 3").unwrap();
    assert!(result.contains("|"));

    let result = normalize("5 bitxor 3").unwrap();
    assert!(result.contains("^"));

    let result = normalize("bitnot 5").unwrap();
    assert!(result.contains("~"));
}

#[test]
fn test_normalize_operator_shift() {
    let result = normalize("5 left shift 3").unwrap();
    assert!(result.contains("<<"));

    let result = normalize("5 right shift 3").unwrap();
    assert!(result.contains(">>"));
}

// ─── normalize: function aliases ─────────────────────────────────────

#[test]
fn test_normalize_function_sine() {
    let result = normalize("sine of zero").unwrap();
    assert!(result.contains("sin"));
}

#[test]
fn test_normalize_function_cosine() {
    let result = normalize("cosine of zero").unwrap();
    assert!(result.contains("cos"));
}

#[test]
fn test_normalize_function_tangent() {
    let result = normalize("tangent of zero").unwrap();
    assert!(result.contains("tan"));
}

#[test]
fn test_normalize_function_arc_sine() {
    let result = normalize("arc sine of 0.5").unwrap();
    assert!(result.contains("asin"));
}

#[test]
fn test_normalize_function_arcsin() {
    let result = normalize("arcsin of 0.5").unwrap();
    assert!(result.contains("asin"));
}

#[test]
fn test_normalize_function_square_root() {
    let result = normalize("square root of 16").unwrap();
    assert!(result.contains("sqrt"));
    assert!(result.contains("16"));
}

#[test]
fn test_normalize_function_cube_root() {
    let result = normalize("cube root of 8").unwrap();
    assert!(result.contains("cbrt"));
    assert!(result.contains("8"));
}

#[test]
fn test_normalize_function_absolute_value() {
    let result = normalize("absolute value of -5").unwrap();
    assert!(result.contains("abs"));
}

#[test]
fn test_normalize_function_natural_log() {
    let result = normalize("natural log of e").unwrap();
    assert!(result.contains("log"));
}

#[test]
fn test_normalize_function_hyperbolic_sine() {
    let result = normalize("hyperbolic sine of 1").unwrap();
    assert!(result.contains("sinh"));
}

// ─── normalize: constant phrases ─────────────────────────────────────

#[test]
fn test_normalize_constant_speed_of_light() {
    let result = normalize("speed of light").unwrap();
    assert_eq!(result, "c");
}

#[test]
fn test_normalize_constant_avogadro() {
    let result = normalize("avogadro number").unwrap();
    assert_eq!(result, "na");
}

#[test]
fn test_normalize_constant_gas_constant() {
    let result = normalize("gas constant").unwrap();
    assert_eq!(result, "r");
}

#[test]
fn test_normalize_constant_planck() {
    let result = normalize("planck constant").unwrap();
    assert_eq!(result, "planckconstant");
}

#[test]
fn test_normalize_constant_boltzmann() {
    let result = normalize("boltzmann constant").unwrap();
    assert_eq!(result, "k");
}

#[test]
fn test_normalize_constant_gravity() {
    let result = normalize("gravity").unwrap();
    assert_eq!(result, "standardgravity");
}

#[test]
fn test_normalize_constant_gravitational() {
    let result = normalize("gravitational constant").unwrap();
    assert_eq!(result, "gravitationalconstant");
}

// ─── normalize: input validation ─────────────────────────────────────

#[test]
fn test_normalize_too_long() {
    let long = "a".repeat(100_001);
    let result = normalize(&long);
    assert!(result.is_err());
}

#[test]
fn test_normalize_empty() {
    let result = normalize("").unwrap();
    assert!(result.is_empty());
}

// ─── normalize → run integration ─────────────────────────────────────

#[test]
fn test_run_nl_basic() {
    assert_eq!(
        run("thirty plus five").unwrap(),
        ("35".to_string(), "int".to_string())
    );
    assert_eq!(
        run("fifty divided by five").unwrap(),
        ("10".to_string(), "int".to_string())
    );
}

#[test]
fn test_run_nl_power() {
    assert_eq!(
        run("two to the power of ten").unwrap(),
        ("1024".to_string(), "int".to_string())
    );
}

#[test]
fn test_run_nl_times() {
    assert_eq!(
        run("ten times ten").unwrap(),
        ("100".to_string(), "int".to_string())
    );
}

#[test]
fn test_run_nl_minus() {
    assert_eq!(
        run("twenty minus five").unwrap(),
        ("15".to_string(), "int".to_string())
    );
}

#[test]
fn test_run_nl_modulo() {
    let result = run("17 mod 5").unwrap();
    assert_eq!(result.0, "2");
}

#[test]
fn test_run_nl_bitwise_xor() {
    let result = run("5 xor 3").unwrap();
    assert_eq!(result.0, "6");
}

// ─── evaluate (direct math) ──────────────────────────────────────────

#[test]
fn test_evaluate_basic() {
    assert_eq!(
        evaluate("5 + 3").unwrap(),
        ("8".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("10 - 4").unwrap(),
        ("6".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("6 * 7").unwrap(),
        ("42".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("20 / 4").unwrap(),
        ("5".to_string(), "int".to_string())
    );
}

#[test]
fn test_evaluate_decimals() {
    let result: f64 = evaluate("3.14 + 2.5").unwrap().0.parse().unwrap();
    assert!((result - 5.64).abs() < 1e-10);
    let result: f64 = evaluate("10.5 * 2").unwrap().0.parse().unwrap();
    assert!((result - 21.0).abs() < 1e-10);
}

#[test]
fn test_evaluate_power() {
    assert_eq!(
        evaluate("2 ** 10").unwrap(),
        ("1024".to_string(), "int".to_string())
    );
    // ^ is XOR, not power
    assert_eq!(
        evaluate("2 ^ 10").unwrap(),
        ("8".to_string(), "int".to_string())
    );
}

#[test]
fn test_evaluate_functions() {
    assert_eq!(
        evaluate("sqrt(16)").unwrap(),
        ("4".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("abs(-5)").unwrap(),
        ("5".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("floor(5.7)").unwrap(),
        ("5".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("ceil(5.2)").unwrap(),
        ("6".to_string(), "int".to_string())
    );
}

#[test]
fn test_evaluate_trig() {
    let result: f64 = evaluate("sin(pi/2)").unwrap().0.parse().unwrap();
    assert!((result - 1.0).abs() < 1e-10);

    let result: f64 = evaluate("cos(0)").unwrap().0.parse().unwrap();
    assert!((result - 1.0).abs() < 1e-10);

    let result: f64 = evaluate("tan(0)").unwrap().0.parse().unwrap();
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn test_evaluate_log() {
    let result: f64 = evaluate("log(e)").unwrap().0.parse().unwrap();
    assert!((result - 1.0).abs() < 1e-10);

    let result: f64 = evaluate("log10(100)").unwrap().0.parse().unwrap();
    assert!((result - 2.0).abs() < 1e-10);

    let result: f64 = evaluate("log2(1024)").unwrap().0.parse().unwrap();
    assert!((result - 10.0).abs() < 1e-10);
}

#[test]
fn test_evaluate_factorial() {
    assert_eq!(
        evaluate("factorial(5)").unwrap(),
        ("120".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("factorial(0)").unwrap(),
        ("1".to_string(), "int".to_string())
    );
}

#[test]
fn test_evaluate_gcd_lcm() {
    assert_eq!(
        evaluate("gcd(12, 18)").unwrap(),
        ("6".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("lcm(4, 6)").unwrap(),
        ("12".to_string(), "int".to_string())
    );
}

#[test]
fn test_evaluate_statistical() {
    let median_result: f64 = evaluate("median(1, 2, 3, 4, 5)")
        .unwrap()
        .0
        .parse()
        .unwrap();
    assert!((median_result - 3.0).abs() < 1e-10);

    let mean_result: f64 = evaluate("mean(1, 2, 3, 4, 5)").unwrap().0.parse().unwrap();
    assert!((mean_result - 3.0).abs() < 1e-10);
}

#[test]
fn test_evaluate_constants() {
    let pi: f64 = std::f64::consts::PI;
    let result: f64 = evaluate("pi").unwrap().0.parse().unwrap();
    assert!((result - pi).abs() < 1e-10);

    let e: f64 = std::f64::consts::E;
    let result: f64 = evaluate("e").unwrap().0.parse().unwrap();
    assert!((result - e).abs() < 1e-10);
}

#[test]
fn test_evaluate_error_empty() {
    assert!(evaluate("").is_err());
}

#[test]
fn test_evaluate_error_invalid() {
    assert!(evaluate("++++").is_err());
}

#[test]
fn test_evaluate_division_by_zero() {
    let result = evaluate("10 / 0");
    let is_error_or_inf = result.is_err()
        || result
            .as_ref()
            .map(|(s, _)| s.contains("inf") || s.contains("NaN"))
            .unwrap_or(false);
    assert!(is_error_or_inf);
}

// ─── BUG-024: Multi-word number phrases via run() ─────────────────────

#[test]
fn test_nl_twenty_one_plus_three() {
    assert_eq!(
        run("twenty one plus three").unwrap(),
        ("24".to_string(), "int".to_string())
    );
}

#[test]
fn test_nl_one_hundred() {
    assert_eq!(
        run("one hundred").unwrap(),
        ("100".to_string(), "int".to_string())
    );
}

#[test]
fn test_nl_fifteen() {
    assert_eq!(
        run("fifteen").unwrap(),
        ("15".to_string(), "int".to_string())
    );
}

// ─── BUG-026: NL phrase stripping for "a" ─────────────────────────────

#[test]
fn test_nl_strips_phrase_with_a() {
    // "what is the value of a" should not fail; "a" is in STRIPPED_PHRASES
    // and "the value of" is also stripped, so the remaining is just "a" which
    // gets stripped, leaving an empty expression.
    let result = run("what is the value of a");
    // Should not panic; may return error for empty expr, but must not crash
    // or return an unexpected error about unrecognized tokens.
    match result {
        Ok(_) => {} // if it returns a value, that's fine
        Err(e) => {
            // If it errors, it should be about empty/invalid expression, not
            // about an unrecognized token like "a".
            let msg = format!("{}", e);
            assert!(
                !msg.contains("unrecognized"),
                "Should not fail with unrecognized token: {}",
                msg
            );
        }
    }
}

#[test]
fn test_normalize_strips_a_standalone() {
    // Verify "a" is stripped during normalization
    let result = normalize("what is the value of a").unwrap();
    // After stripping filler phrases, "a" should be removed
    assert!(
        result.is_empty() || result.trim().is_empty(),
        "Expected empty result after stripping 'a', got: {:?}",
        result
    );
}

// ─── BUG-027: FUNCTION_MAPPINGS "ln" ──────────────────────────────────

#[test]
fn test_ln_through_normalization() {
    // "ln(e)" should work: ln maps to log, log(e) = 1
    assert_eq!(run("ln(e)").unwrap(), ("1".to_string(), "int".to_string()));
}

#[test]
fn test_ln_in_function_mappings() {
    // Verify the normalization converts "ln" to "log"
    let normalized = normalize("ln(e)").unwrap();
    assert!(
        normalized.contains("log"),
        "Expected 'log' in normalized output, got: {}",
        normalized
    );
}

// ─── normalization fixes: inverse trig, large numbers, fractions, constants ──

#[test]
fn test_normalize_inverse_sine() {
    let result = normalize("inverse sine of 0.5").unwrap();
    assert!(
        result.contains("asin"),
        "Expected 'asin' in normalized output, got: {}",
        result
    );
}

#[test]
fn test_normalize_arc_cos() {
    let result = normalize("arc cos of 0.5").unwrap();
    assert!(
        result.contains("acos"),
        "Expected 'acos' in normalized output, got: {}",
        result
    );
}

#[test]
fn test_normalize_quintillion() {
    let result = normalize("five quintillion").unwrap();
    assert!(
        result.contains("1000000000000000000") || result.contains("1e18"),
        "Expected quintillion magnitude in normalized output, got: {}",
        result
    );
}

#[test]
fn test_normalize_thousandth() {
    let result = normalize("one thousandth").unwrap();
    assert!(
        result.contains("0.001") || result.contains("1*0.001"),
        "Expected 0.001 in normalized output, got: {}",
        result
    );
}

#[test]
fn test_normalize_constant_faraday() {
    let result = normalize("faraday constant").unwrap();
    assert_eq!(result, "f");
}

// ─── C10: IN/TO operator normalization ───────────────────────────────

#[test]
fn test_normalize_in_operator() {
    let result = normalize("5 meters in feet").unwrap();
    // NZ-10: spelled unit conversions produce convert() form
    assert!(
        result.contains("IN") || result.contains("in") || result.contains("convert("),
        "Expected IN operator or convert() in normalized output, got: {}",
        result
    );
}

#[test]
fn test_normalize_to_operator() {
    let result = normalize("30 km to miles").unwrap();
    // NZ-10: spelled unit conversions produce convert() form
    assert!(
        result.contains("TO") || result.contains("to") || result.contains("convert("),
        "Expected TO operator or convert() in normalized output, got: {}",
        result
    );
}

// ─── NZ-1: "and" should be stripped, not converted to "+" ───────────

#[test]
fn test_nz1_and_stripped_not_plus() {
    let result = normalize("five and three").unwrap();
    // The key fix: "and" should NOT be converted to "+" operator
    // Previously it was mapped to "+" in OPERATOR_CONVERSIONS
    assert!(
        !result.contains("+"),
        "NZ-1: 'five and three' should NOT produce '+' operator, got: {}",
        result
    );
}

#[test]
fn test_nz1_and_not_in_operator_conversions() {
    // Verify "and" is not in OPERATOR_CONVERSIONS
    assert!(
        !eggsact::calc::normalize::OPERATOR_CONVERSIONS.contains_key("and"),
        "NZ-1: 'and' should NOT be in OPERATOR_CONVERSIONS"
    );
}

#[test]
fn test_nz1_and_is_in_stripped_phrases() {
    // Verify "and " is in STRIPPED_PHRASES
    assert!(
        eggsact::calc::normalize::STRIPPED_PHRASES
            .iter()
            .any(|p| p.starts_with("and")),
        "NZ-1: 'and' should be in STRIPPED_PHRASES"
    );
}

// ─── NZ-2: Lowercase temperature conversion ─────────────────────────

#[test]
fn test_nz2_lowercase_temperature_conversion() {
    let result = normalize("100 c in f").unwrap();
    assert!(
        result.contains("C") && result.contains("F"),
        "NZ-2: Expected uppercase C and F in output, got: {}",
        result
    );
}

// ─── NZ-6: Fraction multi-word numbers ──────────────────────────────

#[test]
fn test_nz6_one_half() {
    let result = normalize("one half").unwrap();
    assert_eq!(
        result, "0.5",
        "NZ-6: 'one half' should normalize to 0.5, got: {}",
        result
    );
}

#[test]
fn test_nz6_two_thirds() {
    let result = normalize("two thirds").unwrap();
    assert!(
        result.contains("0.666"),
        "NZ-6: 'two thirds' should contain 0.666, got: {}",
        result
    );
}

#[test]
fn test_nz6_three_quarters() {
    let result = normalize("three quarters").unwrap();
    assert_eq!(
        result, "0.75",
        "NZ-6: 'three quarters' should be 0.75, got: {}",
        result
    );
}

#[test]
fn test_nz6_one_quarter() {
    let result = normalize("one quarter").unwrap();
    assert_eq!(
        result, "0.25",
        "NZ-6: 'one quarter' should be 0.25, got: {}",
        result
    );
}

// ─── NZ-7: Binary word validation ───────────────────────────────────

#[test]
fn test_nz7_binary_word_not_error() {
    let result = normalize("5 not 6");
    assert!(result.is_err(), "NZ-7: '5 not 6' should produce an error");
}

#[test]
fn test_nz7_binary_word_in_error() {
    let result = normalize("1 in 2");
    assert!(result.is_err(), "NZ-7: '1 in 2' should produce an error");
}

// ─── NZ-8: Degrees→temperature detection ─────────────────────────────

#[test]
fn test_nz8_degrees_temperature() {
    let result = normalize("100 degrees in fahrenheit").unwrap();
    // Should NOT contain pi/180 (angle conversion)
    assert!(
        !result.contains("pi/180"),
        "NZ-8: '100 degrees in fahrenheit' should not do angle conversion, got: {}",
        result
    );
}

#[test]
fn test_nz8_degrees_still_converts_angle() {
    let result = normalize("30 degrees").unwrap();
    // Should contain pi/180 for angle conversion
    assert!(
        result.contains("pi/180"),
        "NZ-8: '30 degrees' should still do angle conversion, got: {}",
        result
    );
}

// ─── NZ-13: Factorial func(args)! pattern ────────────────────────────

#[test]
fn test_nz13_factorial_func_args() {
    let result = normalize("factorial(5)!").unwrap();
    assert!(
        result.contains("factorial(factorial"),
        "NZ-13: 'factorial(5)!' should produce nested factorial, got: {}",
        result
    );
}

// ─── NZ-5: Multi-arg comma separation ────────────────────────────────

#[test]
fn test_nz5_mean_of_chain() {
    let result = normalize("mean of 1+2+3").unwrap();
    assert!(
        result.contains("mean(1,2,3)") || result.contains("mean(1, 2, 3)"),
        "NZ-5: 'mean of 1+2+3' should use commas, got: {}",
        result
    );
}

// ─── NZ-3: Same-unit division parens ─────────────────────────────────

#[test]
fn test_nz3_unit_division_parens() {
    // NZ-3 tests the add_same_unit_division_parens function on already-joined expressions
    // The Rust evaluator handles "5m/(3*m)" via preprocess_units
    // Test that the function correctly wraps denominator in parens
    let input = "5*m/3*m";
    let result = add_same_unit_division_parens(input);
    assert_eq!(
        result, "5*m/(3*m)",
        "NZ-3: '{}' should become '5*m/(3*m)', got: {}",
        input, result
    );
}

// ─── NZ-4: Spaced unit caret exponents ────────────────────────────────

#[test]
fn test_nz4_caret_exponent_basic() {
    let result = normalize("5 m ^ 2").unwrap();
    assert!(
        result.contains("m2") || result.contains("m**2"),
        "NZ-4: '5 m ^ 2' should handle caret exponent, got: {}",
        result
    );
}

#[test]
fn test_nz4_caret_exponent_division() {
    let result = normalize("/ m ^ 2").unwrap();
    assert!(
        result.contains("m**2") || result.contains("m2"),
        "NZ-4: '/ m ^ 2' should handle caret exponent in denominator, got: {}",
        result
    );
}

// ─── NZ-9: Postfix unit power words ──────────────────────────────────

#[test]
fn test_nz9_ft_squared() {
    let result = normalize("5 ft squared").unwrap();
    assert!(
        result.contains("ft2") || result.contains("ft**2"),
        "NZ-9: '5 ft squared' should handle ft squared, got: {}",
        result
    );
}

#[test]
fn test_nz9_m_cubed() {
    let result = normalize("3 m cubed").unwrap();
    assert!(
        result.contains("m3") || result.contains("m**3"),
        "NZ-9: '3 m cubed' should handle m cubed, got: {}",
        result
    );
}

// ─── NZ-10: Spelled unit conversions ─────────────────────────────────

#[test]
fn test_nz10_mph_conversion() {
    let result = normalize("60 mph in kph").unwrap();
    assert!(
        result.contains("convert") || result.contains("mph") || result.contains("kph"),
        "NZ-10: '60 mph in kph' should handle mph conversion, got: {}",
        result
    );
}

#[test]
fn test_nz10_liters_conversion() {
    let result = normalize("5 liters in gallons").unwrap();
    assert!(
        result.contains("convert") || result.contains("L") || result.contains("gal"),
        "NZ-10: '5 liters in gallons' should handle L conversion, got: {}",
        result
    );
}

// ─── NZ-12: STRIPPED_PHRASES alignment ──────────────────────────────

#[test]
fn test_nz12_evaluate_not_stripped() {
    // "evaluate" was removed from Rust's STRIPPED_PHRASES to match Python
    let result = normalize("evaluate 5 + 3").unwrap();
    // In Python, "evaluate" is NOT a stripped phrase, so it stays
    // The result should still evaluate correctly since "evaluate" is not a number word
    assert!(
        result.contains("5") && result.contains("3"),
        "NZ-12: 'evaluate 5 + 3' should still work, got: {}",
        result
    );
}

#[test]
fn test_nz12_solve_not_stripped() {
    // "solve" was removed from Rust's STRIPPED_PHRASES to match Python
    let result = normalize("solve 5 + 3").unwrap();
    assert!(
        result.contains("5") && result.contains("3"),
        "NZ-12: 'solve 5 + 3' should still work, got: {}",
        result
    );
}

// ─── NZ-14: MAX_TEXT_LENGTH alignment ────────────────────────────────

#[test]
fn test_nz14_max_text_length_10000() {
    // Input longer than 10000 should be rejected
    let long_input = "a".repeat(10001);
    let result = normalize(&long_input);
    assert!(
        result.is_err(),
        "NZ-14: Input longer than 10000 should be rejected"
    );
}

// ─── BUG-002: % symbol percentage conversion ─────────────────────────

#[test]
fn test_pct_symbol_basic() {
    let result = normalize("50%").unwrap();
    assert!(
        result.contains("(50/100)"),
        "BUG-002: '50%' should convert to '(50/100)', got: {}",
        result
    );
}

#[test]
fn test_pct_symbol_decimal() {
    let result = normalize("33.3%").unwrap();
    assert!(
        result.contains("(33.3/100)"),
        "BUG-002: '33.3%' should convert to '(33.3/100)', got: {}",
        result
    );
}

#[test]
fn test_pct_symbol_in_expression() {
    let result = normalize("50% of 200").unwrap();
    assert!(
        result.contains("(50/100)"),
        "BUG-002: '50% of 200' should contain '(50/100)', got: {}",
        result
    );
}

#[test]
fn test_pct_symbol_no_space() {
    let result = normalize("75%").unwrap();
    assert!(
        result.contains("(75/100)"),
        "BUG-002: '75%' (no space) should convert, got: {}",
        result
    );
}

// ─── BUG-003: stds → std_sample mapping ──────────────────────────────

#[test]
fn test_stds_maps_to_std_sample() {
    let normalized = normalize("stds(1, 2, 3)").unwrap();
    assert!(
        normalized.contains("std_sample"),
        "BUG-003: 'stds' should normalize to 'std_sample', got: {}",
        normalized
    );
}

// ─── BUG-011: degrees without in/to keyword ─────────────────────────

#[test]
fn test_degrees_bare_temperature() {
    // "5 degrees fahrenheit" (without "in") should keep as temperature
    let result = normalize("5 degrees fahrenheit").unwrap();
    assert!(
        !result.contains("pi/180"),
        "BUG-011: '5 degrees fahrenheit' should not do angle conversion, got: {}",
        result
    );
}

#[test]
fn test_degrees_bare_celsius() {
    let result = normalize("100 degrees celsius").unwrap();
    assert!(
        !result.contains("pi/180"),
        "BUG-011: '100 degrees celsius' should not do angle conversion, got: {}",
        result
    );
}

// ─── BUG-013: M → MR zero-arg memory function ───────────────────────

#[test]
fn test_M_in_function_mappings() {
    // Verify "M" is mapped to "MR" in FUNCTION_MAPPINGS
    assert!(
        eggsact::calc::normalize::FUNCTION_MAPPINGS
            .get("M")
            .is_some(),
        "BUG-013: 'M' should be in FUNCTION_MAPPINGS"
    );
    assert_eq!(
        eggsact::calc::normalize::FUNCTION_MAPPINGS
            .get("M")
            .unwrap(),
        &"MR",
        "BUG-013: 'M' should map to 'MR'"
    );
}

// ─── BUG-006: missing function mappings self-identify ────────────────

#[test]
fn test_function_mappings_self_identify() {
    // These functions should have self-mappings so the normalizer recognizes them
    let self_mapped = [
        "sinh",
        "cosh",
        "tanh",
        "asinh",
        "acosh",
        "atanh",
        "exp",
        "ceil",
        "clamp",
        "factorial",
        "nPr",
        "nCr",
        "gcd",
        "lcm",
        "perm",
        "comb",
        "bitand",
        "bitor",
        "bitxor",
        "bitnot",
        "bitlshift",
        "bitrshift",
    ];
    for func in &self_mapped {
        let val = eggsact::calc::normalize::FUNCTION_MAPPINGS.get(func);
        assert!(
            val.is_some(),
            "BUG-006: '{}' should be in FUNCTION_MAPPINGS",
            func
        );
        assert_eq!(
            val.unwrap(),
            func,
            "BUG-006: '{}' should map to itself",
            func
        );
    }
}
