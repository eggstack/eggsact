use eggsact::calc::{evaluate, run};

fn v(expr: &str) -> String {
    let (result, _) = evaluate(expr).unwrap();
    result
}

fn typ(expr: &str) -> String {
    let (_, t) = evaluate(expr).unwrap();
    t
}

// ─── BUG-001: combine_number_parts compound number multipliers ──────

#[test]
fn test_bug001_twenty_one_thousand() {
    let (result, _) = run("twenty one thousand").unwrap();
    assert_eq!(result, "21000");
}

#[test]
fn test_bug001_twenty_three_hundred() {
    let (result, _) = run("twenty three hundred").unwrap();
    assert_eq!(result, "2300");
}

#[test]
fn test_bug001_fifteen_hundred() {
    let (result, _) = run("fifteen hundred").unwrap();
    assert_eq!(result, "1500");
}

#[test]
fn test_bug001_one_hundred_twenty_one_thousand() {
    let (result, _) = run("one hundred twenty one thousand").unwrap();
    assert_eq!(result, "121000");
}

#[test]
fn test_bug001_multi_scale_compound_number() {
    let (result, _) = run("one million two hundred three thousand four hundred five").unwrap();
    assert_eq!(result, "1203405");
}

#[test]
fn test_bug001_hundred_without_leading_one() {
    let (result, _) = run("hundred twenty one").unwrap();
    assert_eq!(result, "121");
}

#[test]
fn test_bug001_fifty_million() {
    let (result, _) = run("fifty million").unwrap();
    assert_eq!(result, "50000000");
}

// ─── BUG-014: Dead uppercase operators removed ─────────────────────
// Verify that uppercase operators still work since normalize lowercases first.

#[test]
fn test_bug014_uppercase_or() {
    let (result, _) = run("5 OR 3").unwrap();
    // OR has lower precedence than +, but here it's just 5 OR 3
    // 5 | 3 = 7
    assert_eq!(result, "7");
}

#[test]
fn test_bug014_uppercase_xor() {
    let (result, _) = run("5 XOR 3").unwrap();
    // 5 ^ 3 = 6
    assert_eq!(result, "6");
}

#[test]
fn test_bug014_lowercase_or() {
    let (result, _) = run("5 or 3").unwrap();
    assert_eq!(result, "7");
}

#[test]
fn test_bug014_lowercase_xor() {
    let (result, _) = run("5 xor 3").unwrap();
    assert_eq!(result, "6");
}

#[test]
fn test_bug014_mixed_case_or() {
    let (result, _) = run("5 Or 3").unwrap();
    assert_eq!(result, "7");
}

// ─── BUG-015: comb(r > n) returns 0 ────────────────────────────────

#[test]
fn test_bug015_comb_r_greater_than_n() {
    let (result, _) = evaluate("comb(3, 5)").unwrap();
    assert_eq!(result, "0");
}

#[test]
fn test_bug015_comb_equal() {
    let (result, _) = evaluate("comb(5, 5)").unwrap();
    assert_eq!(result, "1");
}

#[test]
fn test_bug015_comb_normal() {
    let (result, _) = evaluate("comb(5, 2)").unwrap();
    assert_eq!(result, "10");
}

#[test]
fn test_bug015_ncr_r_greater_than_n() {
    let (result, _) = evaluate("ncr(2, 10)").unwrap();
    assert_eq!(result, "0");
}

// ─── BUG-016: median NaN errors ────────────────────────────────────

#[test]
fn test_bug016_median_nan_returns_error() {
    let result = run("median(1, 2, nan, 3)");
    assert!(result.is_err(), "median with NaN should return an error");
}

#[test]
fn test_bug016_median_normal() {
    let (result, _) = run("median(1, 2, 3, 4, 5)").unwrap();
    assert_eq!(result, "3");
}

#[test]
fn test_bug016_median_even_count() {
    let (result, _) = run("median(1, 2, 3, 4)").unwrap();
    assert_eq!(result, "2.5");
}

#[test]
fn test_bug016_median_two_values() {
    let (result, _) = run("median(1, 5)").unwrap();
    assert_eq!(result, "3");
}

// ─── BUG-005: `atm` should resolve as a unit, not a constant ────────

#[test]
fn test_bug005_atm_bare_resolves_to_one() {
    let (result, _) = run("atm").unwrap();
    // atm is a unit alias → evaluator returns 1.0; no numeric prefix means
    // preprocess_units does not attach a unit suffix to the output string.
    assert_eq!(result, "1");
}

#[test]
fn test_bug005_atm_times_two() {
    let (result, _) = run("2 * atm").unwrap();
    // BUG-009 / parity B9: the joined-string preprocess_units recognizes
    // `2*atm` as a `<num>*<unit>` segment after split_at_operators(), so
    // `atm` (a known unit alias) is preserved as the unit. Matches Python
    // parity (`2 atm`).
    assert_eq!(result, "2 atm");
}

#[test]
fn test_bug005_atm_is_not_numeric_constant() {
    let (result, _) = run("atm").unwrap();
    // If atm were still in the CONSTANTS map it would resolve to 101325.
    assert_ne!(result, "101325");
    assert_ne!(result, "101325.0");
}

// ─── BUG-009: spaced / compound / per-unit forms ─────────────────────
//
// Regression tests for parity with the Python reference (`eggcalc/`). All
// Python-equivalent results verified against `eggcalc.normalize.run`.

#[test]
fn test_bug009_mixed_unit_arithmetic_mph_kmh() {
    // 60 mph + 60 km/h should add 60 mph + (60 km/h in mph = 37.28 mph)
    // = 97.28 mph (Python parity).
    let (result, _) = run("60 mph + 60 km/h").unwrap();
    assert_eq!(result, "97.28227153424004 mph");
}

#[test]
fn test_bug009_compound_unit_compact_form() {
    let (result, _) = run("30 km/h in mph").unwrap();
    assert_eq!(result, "18.641135767120023 mph");
}

#[test]
fn test_bug009_compound_unit_split_form() {
    // "5 km / h" → 5 km/h (preserve compound unit display)
    let (result, _) = run("5 km / h").unwrap();
    assert_eq!(result, "5 km/h");
}

#[test]
fn test_bug009_per_unit_kilometer() {
    let (result, _) = run("60 kilometer per hour").unwrap();
    assert_eq!(result, "60 km/h");
}

#[test]
fn test_bug009_per_unit_miles() {
    let (result, _) = run("60 miles per hour").unwrap();
    assert_eq!(result, "60 mph");
}

#[test]
fn test_bug009_per_unit_mi_hr() {
    let (result, _) = run("60 mi per hr").unwrap();
    assert_eq!(result, "60 mph");
}

#[test]
fn test_bug009_per_unit_two_occurrences() {
    // Make sure PER_UNIT_RE matches BOTH occurrences of "<num> ... per ...".
    let (result, _) = run("60 miles per hour + 30 km per hour").unwrap();
    // 60 mph + 30 km/h ≈ 60 + 18.64 = 78.64 mph
    assert_eq!(result, "78.64113576712002 mph");
}

#[test]
fn test_bug009_per_unit_minutes() {
    let (result, _) = run("1 mile per minute").unwrap();
    assert_eq!(result, "1 mi/min");
}

#[test]
fn test_bug009_per_unit_meters_second() {
    let (result, _) = run("60 meter per second").unwrap();
    assert_eq!(result, "60 m/s");
}

#[test]
fn test_bug009_bare_simple_unit_mph() {
    let (result, _) = run("60 mph").unwrap();
    assert_eq!(result, "60 mph");
}

#[test]
fn test_bug009_bare_simple_unit_knot() {
    let (result, _) = run("60 knot").unwrap();
    assert_eq!(result, "60 kn");
}

#[test]
fn test_bug009_bare_simple_unit_kph() {
    let (result, _) = run("60 kph in mph").unwrap();
    assert_eq!(result, "37.282271534240046 mph");
}

#[test]
fn test_bug009_unit_conversion_miles_to_kmh() {
    let (result, _) = run("60 miles per hour to km/h").unwrap();
    assert_eq!(result, "96.56063999999999 km/h");
}

#[test]
fn test_bug009_unit_conversion_ms_to_mph() {
    let (result, _) = run("60 m/s to mph").unwrap();
    assert_eq!(result, "134.21617752326415 mph");
}

#[test]
fn test_bug009_mixed_arithmetic_5mph_10mps() {
    let (result, _) = run("5 miles per hour + 10 meters per second").unwrap();
    // 5 mph + 10 m/s ≈ 5 + 22.37 = 27.37 mph
    assert_eq!(result, "27.369362920544024 mph");
}

#[test]
fn test_bug009_meters_feet_conversion() {
    let (result, _) = run("100 m + 50 ft").unwrap();
    // 100 m + 50 ft ≈ 100 + 15.24 = 115.24 m
    assert_eq!(result, "115.24 m");
}

#[test]
fn test_bug009_meters_centimeters() {
    let (result, _) = run("50 m + 25 cm").unwrap();
    // 50 m + 0.25 m = 50.25 m
    assert_eq!(result, "50.25 m");
}

#[test]
fn test_bug009_percent_operator() {
    // 50% = 0.5 (Python parity: no unit suffix in output)
    let (result, _) = run("50%").unwrap();
    assert_eq!(result, "0.5");
}

#[test]
fn test_bug009_percent_not_confused_with_modulo() {
    // Modulo should still work; "17 mod 5" → 2 (NOT "175 %").
    let (result, _) = run("17 mod 5").unwrap();
    assert_eq!(result, "2");
}

#[test]
fn test_bug009_operator_minus_not_a_unit() {
    // Regression: BARE_SIMPLE_UNIT_RE could misread "10 minus" as
    // "<num>=10, unit=m". Word-boundary + length-descending sort fixes this.
    let (result, _) = run("ten minus three").unwrap();
    assert_eq!(result, "7");
}

#[test]
fn test_bug009_unit_alt_longest_first() {
    // Regression: UNIT_ALT must try "miles" before "m" to avoid the engine
    // greedily matching the shorter prefix.
    let (result, _) = run("60 miles").unwrap();
    assert_eq!(result, "60 mi");
}

// ─── BUG-001/BUG-002: factorial range and big-int output ─────────────

#[test]
fn test_bug001_factorial_supports_170() {
    // Python's math.factorial(170) is a 309-digit integer; Rust used to
    // cap at 170 silently returning f64 (losing precision past 170).
    let (val, ty) = evaluate("factorial(170)").unwrap();
    assert_eq!(ty, "int");
    assert!(val.len() > 100);
    assert!(val.starts_with("7257"));
}

#[test]
fn test_bug002_factorial_supports_1000() {
    // MAX_FACTORIAL was raised from 170 → 1000 to match Python's
    // unbounded math.factorial.
    let (val, ty) = evaluate("factorial(1000)").unwrap();
    assert_eq!(ty, "int");
    assert!(val.len() > 2500);
    assert!(val.starts_with("40238726007709377354"));
}

#[test]
fn test_bug002_factorial_overflow_rejected() {
    // Anything past MAX_FACTORIAL must still surface a clean error.
    let r = evaluate("factorial(1001)");
    assert!(r.is_err());
    let msg = r.unwrap_err();
    assert!(msg.contains("out of range"));
}

#[test]
fn test_bug002_factorial_small_values_still_int() {
    // Small inputs must keep returning ints to match Python's behavior.
    assert_eq!(typ("factorial(0)"), "int");
    assert_eq!(typ("factorial(1)"), "int");
    assert_eq!(typ("factorial(5)"), "int");
    assert_eq!(v("factorial(5)"), "120");
}

// ─── BUG-003: polar() with two real args returns (r, phi) tuple ──────

#[test]
fn test_bug003_polar_two_args() {
    let (val, ty) = evaluate("polar(5,1)").unwrap();
    assert_eq!(ty, "str");
    assert_eq!(val, "(5, 1)");
}

#[test]
fn test_bug003_polar_two_args_with_pi() {
    let (val, ty) = evaluate("polar(2,3.14159)").unwrap();
    assert_eq!(ty, "str");
    assert!(val.starts_with("(2,"));
    assert!(val.contains("3.14159"));
}

#[test]
fn test_bug003_polar_one_arg_still_works() {
    // Single-arg form (Python's cmath.polar convention) must still work.
    let (val, ty) = evaluate("polar(-5)").unwrap();
    assert_eq!(ty, "str");
    assert!(val.starts_with("(5,"));
    assert!(val.contains("3.14159"));
}

// ─── BUG-004: rect(r, phi) returns (r*cos(phi), r*sin(phi)) complex ──

#[test]
fn test_bug004_rect_zero_angle() {
    // rect(1, 0) → (1, 0): cos(0)=1, sin(0)=0.
    let (val, ty) = evaluate("rect(1,0)").unwrap();
    assert_eq!(ty, "str");
    assert_eq!(val, "(1, 0)");
}

#[test]
fn test_bug004_rect_pi_yields_negative_real() {
    // rect(2, π) → (2·cos(π), 2·sin(π)) = (-2, ~0).
    let (val, ty) = evaluate("rect(2,3.14159265)").unwrap();
    assert_eq!(ty, "str");
    assert!(val.starts_with("(-2,"));
}

#[test]
fn test_bug004_rect_pi_over_two_yields_imaginary() {
    // rect(1, π/2) → (cos(π/2), sin(π/2)) = (~0, 1).
    let (val, _) = evaluate("rect(1,1.5707963267948966)").unwrap();
    assert!(val.contains("1)"));
    assert!(val.starts_with("(0"));
}

// ─── BUG-204: MAX_RESULT_DIGITS branch was unreachable dead code ────
// After removing the constant, behavior should match Python's: large
// integers within f64 range are accepted; only NaN/Inf/1e308 cause errors.

#[test]
fn test_bug204_no_dead_digit_branch() {
    // BUG-004's existing assertion still holds after the dead branch removal:
    // 21-digit finite values must be accepted.
    let result = evaluate("99999999999999999999 + 1");
    assert!(
        result.is_ok(),
        "BUG-204: 21-digit finite value should be accepted, got error: {:?}",
        result.err()
    );
}

// ─── BUG-205: perm/comb silently lose precision for n > ~170 ────────
// perm/comb must now return exact big-integer results. Small values
// stay as f64; large values surface as a __int_result__ sentinel so
// the MCP layer reports type "int" instead of a rounded float.

#[test]
fn test_bug205_perm_exact_at_overflow_boundary() {
    // perm(1000, 5) = 990034950024000 — fits exactly in f64.
    // Before the fix, big results were silently rounded by f64; the new code
    // returns exact bigint values, which `evaluate` formats as type "int"
    // for any f64 that happens to be integer-valued.
    let (val, ty) = evaluate("perm(1000, 5)").unwrap();
    assert_eq!(
        ty, "int",
        "perm(1000, 5) is an integer-valued result, type should be 'int'"
    );
    assert_eq!(
        val, "990034950024000",
        "BUG-205: perm(1000, 5) should be exactly 990034950024000"
    );
}

#[test]
fn test_bug205_comb_exact_at_53bit_boundary() {
    // comb(1000, 6) is a 17-digit integer — beyond 2^53 mantissa.
    // Old code returned a rounded float; new code returns an int.
    let (val, ty) = evaluate("comb(1000, 6)").unwrap();
    assert_eq!(
        ty, "int",
        "BUG-205: comb(1000, 6) should surface as 'int', got type '{}' val={:?}",
        ty, val
    );
    // Expected: 1000*999*998*997*996*995 / 720.
    let expected: u128 = 1000 * 999 * 998 * 997 * 996 * 995 / 720;
    let actual: u128 = val.parse().expect("int result should parse");
    assert_eq!(
        actual, expected,
        "BUG-205: comb(1000, 6) should be exact"
    );
}

// ─── BUG-206: nextprime/prevprime skip the MAX_PRIME upper-bound guard ───
// Inputs > MAX_PRIME must now produce a hard error rather than running
// an O(sqrt(n)) trial-division loop per candidate.

#[test]
fn test_bug206_nextprime_rejects_above_max_prime() {
    // Use a value well above MAX_PRIME = 10^12.
    let result = evaluate("nextprime(10000000000000)");
    assert!(
        result.is_err(),
        "BUG-206: nextprime(1e13) should be rejected, got: {:?}",
        result.ok()
    );
}

#[test]
fn test_bug206_prevprime_rejects_above_max_prime() {
    let result = evaluate("prevprime(10000000000000)");
    assert!(
        result.is_err(),
        "BUG-206: prevprime(1e13) should be rejected, got: {:?}",
        result.ok()
    );
}

#[test]
fn test_bug206_isprime_still_rejects_above_max_prime() {
    // Regression: isprime already enforced this guard.
    let result = evaluate("isprime(10000000000000)");
    assert!(
        result.is_err(),
        "isprime(1e13) should still be rejected"
    );
}
