use eggsact::calc::{evaluate, run};
use eggsact::mcp::tools::json_canonicalize;
use serde_json::json;

fn val(expr: &str) -> f64 {
    let (result, _) = evaluate(expr).unwrap();
    result.parse::<f64>().unwrap_or(0.0)
}

#[test]
fn test_basic_arithmetic() {
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
fn test_order_of_operations() {
    assert_eq!(
        evaluate("2 + 3 * 4").unwrap(),
        ("14".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("10 - 2 * 3").unwrap(),
        ("4".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("20 / 4 + 3").unwrap(),
        ("8".to_string(), "int".to_string())
    );
}

#[test]
fn test_parentheses() {
    assert_eq!(
        evaluate("(2 + 3) * 4").unwrap(),
        ("20".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("2 * (3 + 4)").unwrap(),
        ("14".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("(10 - 2) * (3 + 1)").unwrap(),
        ("32".to_string(), "int".to_string())
    );
}

#[test]
fn test_power() {
    assert_eq!(
        evaluate("2 ** 3").unwrap(),
        ("8".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("2 ** 10").unwrap(),
        ("1024".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("10 ** 0").unwrap(),
        ("1".to_string(), "int".to_string())
    );
}

#[test]
fn test_modulo() {
    assert_eq!(
        evaluate("17 % 5").unwrap(),
        ("2".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("20 % 4").unwrap(),
        ("0".to_string(), "int".to_string())
    );
}

#[test]
fn test_trigonometric() {
    let result: f64 = evaluate("sin(pi/2)").unwrap().0.parse().unwrap();
    assert!((result - 1.0).abs() < 1e-10);
}

#[test]
fn test_logarithmic() {
    let result: f64 = evaluate("log(e)").unwrap().0.parse().unwrap();
    assert!((result - 1.0).abs() < 1e-10);

    let result: f64 = evaluate("log10(100)").unwrap().0.parse().unwrap();
    assert!((result - 2.0).abs() < 1e-10);

    let result: f64 = evaluate("log2(1024)").unwrap().0.parse().unwrap();
    assert!((result - 10.0).abs() < 1e-10);
}

#[test]
fn test_constants() {
    let pi: f64 = std::f64::consts::PI;
    let result: f64 = evaluate("pi").unwrap().0.parse().unwrap();
    assert!((result - pi).abs() < 1e-10);

    let e: f64 = std::f64::consts::E;
    let result: f64 = evaluate("e").unwrap().0.parse().unwrap();
    assert!((result - e).abs() < 1e-10);
}

#[test]
fn test_factorial() {
    assert_eq!(
        evaluate("factorial(5)").unwrap(),
        ("120".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("factorial(0)").unwrap(),
        ("1".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("factorial(1)").unwrap(),
        ("1".to_string(), "int".to_string())
    );
}

#[test]
fn test_sqrt() {
    assert_eq!(
        evaluate("sqrt(16)").unwrap(),
        ("4".to_string(), "int".to_string())
    );
    let result: f64 = evaluate("sqrt(2)").unwrap().0.parse().unwrap();
    assert!((result - 1.41421356237).abs() < 1e-6);
}

#[test]
fn test_abs() {
    assert_eq!(
        evaluate("abs(-5)").unwrap(),
        ("5".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("abs(3)").unwrap(),
        ("3".to_string(), "int".to_string())
    );
}

#[test]
fn test_floor_ceil() {
    assert_eq!(
        evaluate("floor(3.7)").unwrap(),
        ("3".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("ceil(3.2)").unwrap(),
        ("4".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("round(3.5)").unwrap(),
        ("4".to_string(), "int".to_string())
    );
}

#[test]
fn test_max_min() {
    assert_eq!(
        evaluate("max(1, 5, 3)").unwrap(),
        ("5".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("min(1, 5, 3)").unwrap(),
        ("1".to_string(), "int".to_string())
    );
}

#[test]
fn test_sum() {
    assert_eq!(
        evaluate("sum(1, 2, 3, 4, 5)").unwrap(),
        ("15".to_string(), "int".to_string())
    );
}

#[test]
fn test_negation() {
    assert_eq!(
        evaluate("-5 + 3").unwrap(),
        ("-2".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("-(-3)").unwrap(),
        ("3".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("-100").unwrap(),
        ("-100".to_string(), "int".to_string())
    );
}

#[test]
fn test_negative_in_expressions() {
    assert_eq!(
        evaluate("-5 * -3").unwrap(),
        ("15".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("10 + -3").unwrap(),
        ("7".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("10 - -3").unwrap(),
        ("13".to_string(), "int".to_string())
    );
}

#[test]
fn test_nl_simple() {
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
fn test_nl_power() {
    assert_eq!(
        run("two to the power of ten").unwrap(),
        ("1024".to_string(), "int".to_string())
    );
}

#[test]
fn test_division_by_zero() {
    let result = evaluate("10 / 0");
    let is_error_or_inf_nan = result.is_err()
        || result
            .as_ref()
            .map(|(s, _)| s.contains("inf") || s.contains("NaN"))
            .unwrap_or(false);
    assert!(is_error_or_inf_nan);
}

#[test]
fn test_invalid_expression() {
    let result = evaluate("++++");
    assert!(result.is_err());
}

#[test]
fn test_empty_expression() {
    let result = evaluate("");
    assert!(result.is_err());
}

#[test]
fn test_only_whitespace() {
    let result = evaluate("   ");
    assert!(result.is_err());
}

#[test]
fn test_very_large_numbers() {
    let result = evaluate("10 ** 309");
    // C8/C9: inf results are now errors, not valid output
    assert!(result.is_err());
}

#[test]
fn test_very_small_numbers() {
    let result = evaluate("10 ** -100");
    assert!(result.is_ok());
}

#[test]
fn test_nested_parentheses() {
    assert_eq!(
        evaluate("((2 + 3) * (4 + 5))").unwrap(),
        ("45".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("(((1 + 1) + 1) + 1)").unwrap(),
        ("4".to_string(), "int".to_string())
    );
}

#[test]
fn test_multiple_operations() {
    assert_eq!(
        evaluate("1 + 2 + 3 + 4").unwrap(),
        ("10".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("10 - 1 - 2 - 3").unwrap(),
        ("4".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("2 * 3 * 4").unwrap(),
        ("24".to_string(), "int".to_string())
    );
}

#[test]
fn test_statistical_functions() {
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
fn test_gcd_lcm() {
    assert_eq!(
        evaluate("gcd(12, 18)").unwrap(),
        ("6".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("gcd(7, 13)").unwrap(),
        ("1".to_string(), "int".to_string())
    );
    assert_eq!(
        evaluate("lcm(4, 6)").unwrap(),
        ("12".to_string(), "int".to_string())
    );
}

#[test]
fn test_number_words_complex() {
    assert_eq!(
        run("ten times ten").unwrap(),
        ("100".to_string(), "int".to_string())
    );
    assert_eq!(
        run("twenty plus thirty").unwrap(),
        ("50".to_string(), "int".to_string())
    );
}

// ─── Regression: missing closing paren ───────────────────────────────

#[test]
fn test_missing_closing_paren() {
    let result = evaluate("sin(1");
    assert!(result.is_err());
}

// ─── Regression: pow() negative base with fractional exponent ────────

#[test]
fn test_pow_negative_base_fractional_exponent() {
    let result = evaluate("pow(-2, 0.5)");
    assert!(result.is_err());
}

// ─── Regression: bitwise with fractional operand ─────────────────────

#[test]
fn test_bitwise_fractional_error() {
    let result = evaluate("1.5 & 2");
    assert!(result.is_err());
}

// ─── BUG-021: Negative modulo (Python-style floored) ─────────────────

#[test]
fn test_negative_modulo_minus7_mod_2() {
    assert_eq!(
        evaluate("-7 % 2").unwrap(),
        ("1".to_string(), "int".to_string())
    );
}

#[test]
fn test_negative_modulo_minus7_mod_minus2() {
    assert_eq!(
        evaluate("-7 % -2").unwrap(),
        ("-1".to_string(), "int".to_string())
    );
}

#[test]
fn test_negative_modulo_7_mod_minus2() {
    assert_eq!(
        evaluate("7 % -2").unwrap(),
        ("-1".to_string(), "int".to_string())
    );
}

#[test]
fn test_negative_modulo_7_mod_2() {
    assert_eq!(
        evaluate("7 % 2").unwrap(),
        ("1".to_string(), "int".to_string())
    );
}

#[test]
fn test_negative_modulo_0_mod_5() {
    assert_eq!(
        evaluate("0 % 5").unwrap(),
        ("0".to_string(), "int".to_string())
    );
}

#[test]
fn test_negative_modulo_minus1_mod_1() {
    assert_eq!(
        evaluate("-1 % 1").unwrap(),
        ("0".to_string(), "int".to_string())
    );
}

// ─── BUG-022: gcd/lcm integer validation ─────────────────────────────

#[test]
fn test_gcd_rejects_non_integer_float() {
    let result = evaluate("gcd(5.5, 3)");
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(
        msg.contains("integer"),
        "Expected 'integer' in error: {}",
        msg
    );
}

#[test]
fn test_lcm_rejects_non_integer_float() {
    let result = evaluate("lcm(1.5, 2)");
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(
        msg.contains("integer"),
        "Expected 'integer' in error: {}",
        msg
    );
}

#[test]
fn test_gcd_with_integers() {
    assert_eq!(
        evaluate("gcd(6, 4)").unwrap(),
        ("2".to_string(), "int".to_string())
    );
}

#[test]
fn test_lcm_with_integers() {
    assert_eq!(
        evaluate("lcm(6, 4)").unwrap(),
        ("12".to_string(), "int".to_string())
    );
}

// ─── C5: Gravitational constant G ───────────────────────────────────

#[test]
fn test_gravitational_constant_G() {
    let (val, _) = evaluate("G").unwrap();
    let v: f64 = val.parse().unwrap();
    assert!(
        (v - 6.67430e-11).abs() < 1e-15,
        "G should be gravitational constant, got {}",
        v
    );
}

// ─── C6: bare g is gram, not gravity ────────────────────────────────

#[test]
fn test_bare_g_is_gram_not_gravity() {
    let result = evaluate("g");
    // g should be 1.0 (gram unit) not 9.80665 (gravity)
    if let Ok((val, _)) = result {
        let v: f64 = val.parse().unwrap();
        assert!(
            (v - 1.0).abs() < 1e-10,
            "bare 'g' should be 1 (gram), got {}",
            v
        );
    }
}

// ─── EV-2: Gas constant R accessible via uppercase ──────────────────

#[test]
fn test_gas_constant_R_uppercase() {
    let (val, _) = evaluate("R").unwrap();
    let v: f64 = val.parse().unwrap();
    assert!(
        (v - 8.314462618).abs() < 1e-6,
        "R should be gas constant 8.314462618, got {}",
        v
    );
}

#[test]
fn test_gas_constant_r_lowercase() {
    let (val, _) = evaluate("r").unwrap();
    let v: f64 = val.parse().unwrap();
    assert!(
        (v - 8.314462618).abs() < 1e-6,
        "r should be gas constant 8.314462618, got {}",
        v
    );
}

// ─── C8/C9: NaN and inf are errors ──────────────────────────────────

#[test]
fn test_nan_is_error() {
    let result = evaluate("0.0 / 0.0");
    assert!(result.is_err(), "0/0 should return error, not NaN");
}

#[test]
fn test_inf_is_error() {
    let result = evaluate("1.0 / 0.0");
    assert!(result.is_err(), "1/0 should return error, not inf");
}

// ─── BUG-006: Complex number function tests ─────────────────────────

#[test]
fn test_complex_number_functions() {
    // real()
    assert_eq!(val("real(5)"), 5.0);
    assert_eq!(val("real(-3.14)"), -3.14);

    // imag() - always returns 0 for real numbers
    assert_eq!(val("imag(5)"), 0.0);
    assert_eq!(val("imag(-3.14)"), 0.0);

    // conj() / conjugate() - identity for real numbers
    assert_eq!(val("conj(5)"), 5.0);
    assert_eq!(val("conjugate(-3.14)"), -3.14);

    // phase() - 0 for positive, PI for negative
    assert_eq!(val("phase(5)"), 0.0);
    assert!((val("phase(-5)") - std::f64::consts::PI).abs() < 1e-10);
    assert_eq!(val("phase(0)"), 0.0);

    // polar() - returns string result
    let r = evaluate("polar(5)").unwrap();
    assert!(r.0.contains("5")); // Should contain "5" in the result

    // rect() - returns string result
    let r = evaluate("rect(3, 4)").unwrap();
    assert!(r.0.contains("3") && r.0.contains("4"));
}

// ─── BUG-007: Random function tests ────────────────────────────────

#[test]
fn test_random_functions() {
    // seed() + random() for determinism
    let _ = evaluate("seed(42)");
    let r1 = val("random()");
    assert!(r1 >= 0.0 && r1 < 1.0, "random() should be in [0, 1)");

    // Seed again for same sequence
    let _ = evaluate("seed(42)");
    let r2 = val("random()");
    assert_eq!(r1, r2, "seed() should make random() deterministic");

    // randint(a, b) returns integer in [a, b]
    let _ = evaluate("seed(42)");
    let r = val("randint(1, 10)");
    assert!(
        r >= 1.0 && r <= 10.0 && r == r.floor(),
        "randint(1,10) should be integer in [1,10]"
    );

    // uniform(a, b) returns value in [a, b]
    let _ = evaluate("seed(42)");
    let r = val("uniform(5.0, 10.0)");
    assert!(r >= 5.0 && r <= 10.0, "uniform(5,10) should be in [5,10]");

    // randn() returns finite value
    let _ = evaluate("seed(42)");
    let r = val("randn()");
    assert!(r.is_finite(), "randn() should be finite");

    // gauss(mu, sigma) returns finite value
    let _ = evaluate("seed(42)");
    let r = val("gauss(0, 1)");
    assert!(r.is_finite(), "gauss(0,1) should be finite");
}

// ─── BUG-008: Memory/variable function tests ───────────────────────

#[test]
fn test_memory_and_variable_functions() {
    // store() + recall()
    let _ = evaluate("store(42)");
    assert_eq!(val("recall()"), 42.0);

    // store(value, register) + recall(register)
    let _ = evaluate("store(99, 1)");
    assert_eq!(val("recall(1)"), 99.0);

    // mr() alias for recall()
    let _ = evaluate("store(77)");
    assert_eq!(val("mr()"), 77.0);

    // mplus() adds to memory
    let _ = evaluate("store(10)");
    let _ = evaluate("mplus(5)");
    assert_eq!(val("recall()"), 15.0);

    // mminus() subtracts from memory
    let _ = evaluate("store(10)");
    let _ = evaluate("mminus(3)");
    assert_eq!(val("recall()"), 7.0);

    // mc() clears memory
    let _ = evaluate("store(42)");
    let _ = evaluate("mc()");
    assert_eq!(val("recall()"), 0.0);

    // setvar() + getvar() — uses numeric variable IDs (not string names)
    let _ = evaluate("setvar(10, 1)");
    assert_eq!(val("getvar(1)"), 10.0);

    // getvar() returns 0 for unknown
    assert_eq!(val("getvar(999)"), 0.0);

    // delvar()
    let _ = evaluate("setvar(20, 2)");
    let _ = evaluate("delvar(2)");
    assert_eq!(val("getvar(2)"), 0.0);
}

// ─── BUG-011: format_result i64 boundary tests ─────────────────────

#[test]
fn test_format_result_i64_boundary() {
    // Values that were previously displayed as float (1e15) should now be int
    let r = evaluate("1000000000000000").unwrap();
    assert_eq!(r.1, "int", "1e15 should be formatted as int");

    // Large value still within i64 range
    let r = evaluate("9223372036854775807").unwrap(); // i64::MAX
    assert_eq!(r.1, "int", "i64::MAX should be formatted as int");

    // Fractional value should be float
    let r = evaluate("1.5").unwrap();
    assert_eq!(r.1, "float", "1.5 should be formatted as float");

    // Negative value within i64 range
    let r = evaluate("-9223372036854775808").unwrap(); // i64::MIN
    assert_eq!(r.1, "int", "i64::MIN should be formatted as int");
}

// ─── BUG-012: perm/comb precision tests ─────────────────────────────

#[test]
fn test_perm_comb_precision() {
    // Large values that benefit from multiplicative loop
    assert_eq!(val("perm(1000, 2)"), 999000.0);
    assert_eq!(val("comb(1000, 2)"), 499500.0);

    // Edge cases
    assert_eq!(val("perm(10, 0)"), 1.0);
    assert_eq!(val("comb(10, 0)"), 1.0);
    assert_eq!(val("comb(0, 0)"), 1.0);
    assert_eq!(val("perm(10, 1)"), 10.0);
    assert_eq!(val("comb(10, 10)"), 1.0);
    assert_eq!(val("perm(10, 10)"), 3628800.0); // 10!

    // comb(n, r) = comb(n, n-r)
    assert_eq!(val("comb(10, 3)"), val("comb(10, 7)"));
}

// ─── BUG-002/003: convert/temp end-to-end tests ────────────────────

#[test]
fn test_convert_and_temp_through_run() {
    // Simple unit conversion — convert expects value*unit, target format
    let (result, _) = run("convert(1000*m,km)").unwrap();
    assert_eq!(result, "1 km");

    // Temperature conversion — temp expects (value,from,to)
    let (result, _) = run("temp(100,C,F)").unwrap();
    assert!(
        result.contains("212"),
        "100C should be 212F, got: {}",
        result
    );

    // Spelled conversion
    let (result, _) = run("30 km/h in mph").unwrap();
    assert!(!result.is_empty(), "30 km/h in mph should produce a result");
}

// ─── BUG-013: JSON escape handling tests ────────────────────────────

#[test]
fn test_json_canonicalize_escape_handling() {
    // String containing escaped quotes that look like key-value pairs should not produce false duplicates
    let input = r#"{"key": "value with \"a\": 1 inside"}"#;
    let resp = json_canonicalize(&json!({"text": input}));
    let result_val = resp.result.unwrap();
    let duplicate_keys = result_val.get("duplicate_keys").unwrap();
    // Should not report any duplicates
    assert!(
        duplicate_keys.as_array().unwrap().is_empty(),
        "Escaped quotes in string should not be detected as keys: {:?}",
        duplicate_keys
    );
}

// ─── Constants in expressions ────────────────────────────────────────

#[test]
fn test_pi_times_2() {
    let (val, _) = evaluate("pi * 2").unwrap();
    let v: f64 = val.parse().unwrap();
    let expected = std::f64::consts::PI * 2.0;
    assert!(
        (v - expected).abs() < 1e-10,
        "pi * 2 should be {}, got {}",
        expected,
        v
    );
}

#[test]
fn test_e_squared() {
    let (val, _) = evaluate("e ** 2").unwrap();
    let v: f64 = val.parse().unwrap();
    let expected = std::f64::consts::E.powi(2);
    assert!(
        (v - expected).abs() < 1e-10,
        "e ** 2 should be {}, got {}",
        expected,
        v
    );
}

#[test]
fn test_speed_of_light_times_2() {
    let (val, _) = evaluate("c * 2").unwrap();
    let v: f64 = val.parse().unwrap();
    let expected = 299792458.0 * 2.0;
    assert!(
        (v - expected).abs() < 1.0,
        "c * 2 should be {}, got {}",
        expected,
        v
    );
}

#[test]
fn test_planck_constant_division() {
    let (val, _) = evaluate("planck / (2 * pi)").unwrap();
    let v: f64 = val.parse().unwrap();
    let expected = 6.62607015e-34 / (2.0 * std::f64::consts::PI);
    assert!(
        (v - expected).abs() < 1e-43,
        "planck / (2 * pi) should be {}, got {}",
        expected,
        v
    );
}

#[test]
fn test_avogadro_times_10() {
    let (val, _) = evaluate("na * 10").unwrap();
    let v: f64 = val.parse().unwrap();
    let expected = 6.02214076e23 * 10.0;
    assert!(
        (v - expected).abs() < 1e15,
        "na * 10 should be {}, got {}",
        expected,
        v
    );
}

#[test]
fn test_golden_ratio_phi() {
    let (val, _) = evaluate("phi").unwrap();
    let v: f64 = val.parse().unwrap();
    let expected = 1.618_033_988_749_895;
    assert!(
        (v - expected).abs() < 1e-12,
        "phi should be {}, got {}",
        expected,
        v
    );
}

// ─── Unit calculator integration (run() pipeline) ──────────────────

#[test]
fn test_unit_addition_with_conversion() {
    let (result, _) = run("5m + 3km").unwrap();
    assert!(
        result.contains("3005"),
        "5m + 3km should be ~3005m, got: {}",
        result
    );
}

#[test]
fn test_unit_power() {
    let (result, _) = run("5m ** 2").unwrap();
    assert!(
        result.contains("25"),
        "5m ** 2 should be 25 m**2, got: {}",
        result
    );
    assert!(
        result.contains("m"),
        "Result should contain unit 'm', got: {}",
        result
    );
}

#[test]
fn test_unit_division_by_number() {
    let (result, _) = run("10m / 2").unwrap();
    assert!(
        result.contains("5"),
        "10m / 2 should be 5m, got: {}",
        result
    );
    assert!(
        result.contains("m"),
        "Result should contain 'm', got: {}",
        result
    );
}

#[test]
fn test_number_times_unit() {
    let (result, _) = run("3 * 5m").unwrap();
    assert!(
        result.contains("15"),
        "3 * 5m should be 15m, got: {}",
        result
    );
    assert!(
        result.contains("m"),
        "Result should contain 'm', got: {}",
        result
    );
}

#[test]
fn test_unit_conversion_in_expression() {
    let (result, _) = run("convert(1000*m,km)").unwrap();
    assert!(
        result.contains("1"),
        "1000m in km should be 1, got: {}",
        result
    );
    assert!(
        result.contains("km"),
        "Result should contain 'km', got: {}",
        result
    );
}

#[test]
fn test_temperature_conversion() {
    let (result, _) = run("temp(100,C,F)").unwrap();
    assert!(
        result.contains("212"),
        "100C should be 212F, got: {}",
        result
    );
}

// ─── BUG-001: `h` Planck constant removed ───────────────────────────

#[test]
fn test_h_bare_is_hours_unit() {
    // "h" should resolve to 1.0 (hours unit), not 6.626e-34 (Planck constant)
    let result = evaluate("h");
    if let Ok((val, _)) = result {
        let v: f64 = val.parse().unwrap();
        assert!(
            (v - 1.0).abs() < 1e-10,
            "BUG-001: bare 'h' should be 1 (hours unit), not Planck constant, got {}",
            v
        );
    }
}

#[test]
fn test_planck_still_accessible() {
    // "planck" should still resolve to 6.62607015e-34
    let (val, _) = evaluate("planck").unwrap();
    let v: f64 = val.parse().unwrap();
    assert!(
        (v - 6.62607015e-34).abs() < 1e-43,
        "BUG-001: 'planck' should still be 6.62607015e-34, got {}",
        v
    );
}

// ─── BUG-004: MAX_RESULT_DIGITS check ───────────────────────────────

#[test]
fn test_max_result_digits_large_integer_ok() {
    // 21 digits is well under 10000
    let result = evaluate("99999999999999999999 + 1");
    assert!(
        result.is_ok(),
        "BUG-004: 21-digit result should be accepted"
    );
}

#[test]
fn test_max_result_digits_extreme_overflow() {
    // 10 ** 309 produces Infinity, which is caught by MAX_RESULT_VALUE
    let result = evaluate("10 ** 309");
    assert!(result.is_err(), "BUG-004: 10^309 should overflow");
}
