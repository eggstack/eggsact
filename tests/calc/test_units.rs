use eggsact::calc::units::{
    convert_temperature, get_conversion_factor, get_unit_info, is_unit, UnitValue, UNIT_ALIASES,
    UNIT_BASE,
};

// ─── UnitValue construction ──────────────────────────────────────────

#[test]
fn test_unit_value_new() {
    let uv = UnitValue::new(5.0).unwrap();
    assert_eq!(uv.value, 5.0);
    assert!(uv.unit.is_none());
}

#[test]
fn test_unit_value_with_unit() {
    let uv = UnitValue::with_unit(5.0, "m").unwrap();
    assert_eq!(uv.value, 5.0);
    assert_eq!(uv.unit.as_deref(), Some("m"));
}

#[test]
fn test_unit_value_new_nan() {
    assert!(UnitValue::new(f64::NAN).is_err());
}

#[test]
fn test_unit_value_new_infinity() {
    assert!(UnitValue::new(f64::INFINITY).is_err());
}

#[test]
fn test_unit_value_with_unit_nan() {
    assert!(UnitValue::with_unit(f64::NAN, "m").is_err());
}

// ─── UnitValue arithmetic ────────────────────────────────────────────

#[test]
fn test_unit_value_add_same_unit() {
    let a = UnitValue::with_unit(5.0, "m").unwrap();
    let b = UnitValue::with_unit(3.0, "m").unwrap();
    let c = (a + b).unwrap();
    assert_eq!(c.value, 8.0);
    assert_eq!(c.unit.as_deref(), Some("m"));
}

#[test]
fn test_unit_value_add_incompatible() {
    let a = UnitValue::with_unit(5.0, "m").unwrap();
    let b = UnitValue::with_unit(3.0, "kg").unwrap();
    assert!((a + b).is_err());
}

#[test]
fn test_unit_value_sub_same_unit() {
    let a = UnitValue::with_unit(10.0, "m").unwrap();
    let b = UnitValue::with_unit(3.0, "m").unwrap();
    let c = (a - b).unwrap();
    assert_eq!(c.value, 7.0);
    assert_eq!(c.unit.as_deref(), Some("m"));
}

#[test]
fn test_unit_value_mul_same_unit() {
    let a = UnitValue::with_unit(5.0, "m").unwrap();
    let b = UnitValue::with_unit(5.0, "m").unwrap();
    let c = a * b;
    assert_eq!(c.value, 25.0);
    assert_eq!(c.unit.as_deref(), Some("m**2"));
}

#[test]
fn test_unit_value_mul_different_units() {
    let a = UnitValue::with_unit(5.0, "m").unwrap();
    let b = UnitValue::with_unit(3.0, "s").unwrap();
    let c = a * b;
    assert_eq!(c.value, 15.0);
    assert_eq!(c.unit.as_deref(), Some("m*s"));
}

#[test]
fn test_unit_value_mul_unitless() {
    let a = UnitValue::with_unit(5.0, "m").unwrap();
    let b = UnitValue::new(3.0).unwrap();
    let c = a * b;
    assert_eq!(c.value, 15.0);
    assert_eq!(c.unit.as_deref(), Some("m"));
}

#[test]
fn test_unit_value_div_same_unit() {
    let a = UnitValue::with_unit(10.0, "m").unwrap();
    let b = UnitValue::with_unit(5.0, "m").unwrap();
    let c = a / b;
    assert_eq!(c.value, 2.0);
    assert!(c.unit.is_none());
}

#[test]
fn test_unit_value_div_different_units() {
    let a = UnitValue::with_unit(10.0, "m").unwrap();
    let b = UnitValue::with_unit(2.0, "s").unwrap();
    let c = a / b;
    assert_eq!(c.value, 5.0);
    assert_eq!(c.unit.as_deref(), Some("m/s"));
}

#[test]
fn test_unit_value_div_by_zero() {
    let a = UnitValue::with_unit(10.0, "m").unwrap();
    let b = UnitValue::with_unit(0.0, "s").unwrap();
    let result = a / b;
    assert!(result.value.is_infinite());
    assert_eq!(result.unit.as_deref(), Some("m/s"));
}

// ─── UnitValue conversion ────────────────────────────────────────────

#[test]
fn test_unit_value_convert_to() {
    let a = UnitValue::with_unit(1.0, "km").unwrap();
    let b = a.convert_to("m").unwrap();
    assert_eq!(b.value, 1000.0);
    assert_eq!(b.unit.as_deref(), Some("m"));
}

#[test]
fn test_unit_value_convert_no_unit() {
    let a = UnitValue::new(5.0).unwrap();
    assert!(a.convert_to("m").is_err());
}

// ─── get_conversion_factor ───────────────────────────────────────────

#[test]
fn test_conversion_factor_identity() {
    let factor = get_conversion_factor("m", "m").unwrap();
    assert_eq!(factor, 1.0);
}

#[test]
fn test_conversion_factor_km_to_m() {
    let factor = get_conversion_factor("km", "m").unwrap();
    assert_eq!(factor, 1000.0);
}

#[test]
fn test_conversion_factor_m_to_km() {
    let factor = get_conversion_factor("m", "km").unwrap();
    assert!((factor - 0.001).abs() < 1e-10);
}

#[test]
fn test_conversion_factor_inch_to_mm() {
    let factor = get_conversion_factor("in", "mm").unwrap();
    assert!((factor - 25.4).abs() < 1e-10);
}

#[test]
fn test_conversion_factor_mile_to_km() {
    let factor = get_conversion_factor("mi", "km").unwrap();
    assert!((factor - 1.609344).abs() < 1e-6);
}

#[test]
fn test_conversion_factor_foot_to_m() {
    let factor = get_conversion_factor("ft", "m").unwrap();
    assert!((factor - 0.3048).abs() < 1e-10);
}

#[test]
fn test_conversion_factor_incompatible() {
    assert!(get_conversion_factor("m", "kg").is_err());
}

#[test]
fn test_conversion_factor_unknown_unit() {
    assert!(get_conversion_factor("m", "frobnicate").is_err());
}

// ─── is_unit ─────────────────────────────────────────────────────────

#[test]
fn test_is_unit_meter() {
    assert!(is_unit("m"));
    assert!(is_unit("km"));
    assert!(is_unit("cm"));
    assert!(is_unit("mm"));
}

#[test]
fn test_is_unit_length() {
    assert!(is_unit("ft"));
    assert!(is_unit("mi"));
    assert!(is_unit("yd"));
}

#[test]
fn test_is_unit_mass() {
    assert!(is_unit("kg"));
    assert!(is_unit("g"));
    assert!(is_unit("lb"));
    assert!(is_unit("oz"));
}

#[test]
fn test_is_unit_time() {
    assert!(is_unit("s"));
    assert!(is_unit("min"));
    assert!(is_unit("h"));
    assert!(is_unit("d"));
}

#[test]
fn test_is_unit_temperature() {
    assert!(is_unit("C"));
    assert!(is_unit("F"));
    assert!(is_unit("K"));
}

#[test]
fn test_is_unit_not_unit() {
    assert!(!is_unit("foobar"));
    assert!(!is_unit(""));
    assert!(!is_unit("hello"));
}

// ─── BUG-207: is_unit("b") must resolve to *bit*, not *byte* ──────────
// Lowercase "b" is the SI symbol for bit and must be distinguished from
// uppercase "B" (byte). The fallback to_uppercase used to alias "b" → "B".

#[test]
fn test_bug207_is_unit_lowercase_b_is_bit() {
    assert!(is_unit("b"), "lowercase 'b' is the SI symbol for bit");
    assert!(is_unit("B"), "uppercase 'B' is the SI symbol for byte");
    assert_ne!(
        get_unit_info("b").unwrap().0,
        get_unit_info("B").unwrap().0,
        "BUG-207: 'b' and 'B' must resolve to different units"
    );
    assert_eq!(get_unit_info("b").unwrap().0, "bit");
    assert_eq!(get_unit_info("B").unwrap().0, "B");
}

// ─── get_unit_info ───────────────────────────────────────────────────

#[test]
fn test_get_unit_info_meter() {
    let info = get_unit_info("m").unwrap();
    // Returns (unit_name, category)
    assert_eq!(info.1, "length");
}

#[test]
fn test_get_unit_info_km() {
    let info = get_unit_info("km").unwrap();
    assert_eq!(info.1, "length");
}

#[test]
fn test_get_unit_info_kg() {
    let info = get_unit_info("kg").unwrap();
    assert_eq!(info.1, "mass");
}

#[test]
fn test_get_unit_info_unknown() {
    assert!(get_unit_info("frobnicate").is_none());
}

// ─── UNIT_ALIASES ────────────────────────────────────────────────────

#[test]
fn test_unit_aliases_include_common() {
    assert!(UNIT_ALIASES.contains_key("meter"));
    assert!(UNIT_ALIASES.contains_key("metre"));
    assert!(UNIT_ALIASES.contains_key("kilogram"));
    assert!(UNIT_ALIASES.contains_key("second"));
    assert!(UNIT_ALIASES.contains_key("minute"));
    assert!(UNIT_ALIASES.contains_key("hour"));
}

#[test]
fn test_unit_aliases_abbreviations() {
    assert!(UNIT_ALIASES.contains_key("km"));
    assert!(UNIT_ALIASES.contains_key("m"));
    assert!(UNIT_ALIASES.contains_key("ft"));
    assert!(UNIT_ALIASES.contains_key("mi"));
}

#[test]
fn test_unit_aliases_british_spellings() {
    assert!(UNIT_ALIASES.contains_key("metre"));
    assert!(UNIT_ALIASES.contains_key("metres"));
    assert!(UNIT_ALIASES.contains_key("litre"));
    assert!(UNIT_ALIASES.contains_key("litres"));
    assert!(UNIT_ALIASES.contains_key("kilometre"));
}

// ─── UNIT_BASE ───────────────────────────────────────────────────────

#[test]
fn test_unit_base_length_category() {
    let m = UNIT_BASE.get("m").unwrap();
    assert_eq!(m.category, "length");
    assert_eq!(m.to_base, 1.0);

    let km = UNIT_BASE.get("km").unwrap();
    assert_eq!(km.category, "length");
    assert_eq!(km.to_base, 1000.0);
}

#[test]
fn test_unit_base_mass_category() {
    let kg = UNIT_BASE.get("kg").unwrap();
    assert_eq!(kg.category, "mass");

    let g = UNIT_BASE.get("g").unwrap();
    assert_eq!(g.category, "mass");
    assert_eq!(g.to_base, 0.001);
}

#[test]
fn test_unit_base_time_category() {
    let s = UNIT_BASE.get("s").unwrap();
    assert_eq!(s.category, "time");

    let min = UNIT_BASE.get("min").unwrap();
    assert_eq!(min.to_base, 60.0);

    let h = UNIT_BASE.get("h").unwrap();
    assert_eq!(h.to_base, 3600.0);
}

#[test]
fn test_unit_base_temperature() {
    let c = UNIT_BASE.get("C").unwrap();
    assert_eq!(c.category, "temperature");

    let f = UNIT_BASE.get("F").unwrap();
    assert_eq!(f.category, "temperature");

    let k = UNIT_BASE.get("K").unwrap();
    assert_eq!(k.category, "temperature");
}

// ─── temperature conversions ─────────────────────────────────────────

#[test]
fn test_temperature_c_to_f() {
    let result = convert_temperature(100.0, "C", "F").unwrap();
    assert!((result - 212.0).abs() < 1e-10);
}

#[test]
fn test_temperature_f_to_c() {
    let result = convert_temperature(32.0, "F", "C").unwrap();
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn test_temperature_c_to_k() {
    let result = convert_temperature(0.0, "C", "K").unwrap();
    assert!((result - 273.15).abs() < 1e-10);
}

#[test]
fn test_temperature_k_to_c() {
    let result = convert_temperature(273.15, "K", "C").unwrap();
    assert!((result - 0.0).abs() < 1e-10);
}

#[test]
fn test_temperature_f_to_k() {
    let result = convert_temperature(32.0, "F", "K").unwrap();
    assert!((result - 273.15).abs() < 1e-10);
}

#[test]
fn test_temperature_k_to_f() {
    let result = convert_temperature(273.15, "K", "F").unwrap();
    assert!((result - 32.0).abs() < 1e-10);
}

#[test]
fn test_temperature_zero_k() {
    let result = convert_temperature(0.0, "K", "C").unwrap();
    assert!((result - (-273.15)).abs() < 1e-10);
}

#[test]
fn test_temperature_boiling_f() {
    let result = convert_temperature(212.0, "F", "C").unwrap();
    assert!((result - 100.0).abs() < 1e-10);
}

#[test]
fn test_temperature_negative_c() {
    let result = convert_temperature(-40.0, "C", "F").unwrap();
    assert!((result - (-40.0)).abs() < 1e-10);
}

#[test]
fn test_temperature_identity() {
    let result = convert_temperature(100.0, "C", "C").unwrap();
    assert!((result - 100.0).abs() < 1e-10);
}

#[test]
fn test_temperature_nan_returns_error() {
    let result = convert_temperature(f64::NAN, "C", "F");
    assert!(result.is_err(), "NaN temperature should return an error");
}

#[test]
fn test_temperature_infinity_returns_error() {
    let result = convert_temperature(f64::INFINITY, "C", "F");
    assert!(
        result.is_err(),
        "Infinity temperature should return an error"
    );
}

#[test]
fn test_temperature_negative_infinity_returns_error() {
    let result = convert_temperature(f64::NEG_INFINITY, "C", "F");
    assert!(
        result.is_err(),
        "Negative infinity temperature should return an error"
    );
}

// ─── prefixed units ──────────────────────────────────────────────────

#[test]
fn test_prefixed_unit_kn() {
    let factor = get_conversion_factor("kN", "N").unwrap();
    assert_eq!(factor, 1000.0);
}

#[test]
fn test_prefixed_unit_mv() {
    let factor = get_conversion_factor("mV", "V").unwrap();
    assert!((factor - 0.001).abs() < 1e-10);
}

#[test]
fn test_prefixed_unit_ma() {
    let factor = get_conversion_factor("mA", "A").unwrap();
    assert!((factor - 0.001).abs() < 1e-10);
}

#[test]
fn test_prefixed_unit_kw() {
    let factor = get_conversion_factor("kW", "W").unwrap();
    assert_eq!(factor, 1000.0);
}

// ─── unit aliases roundtrip ──────────────────────────────────────────

#[test]
fn test_unit_alias_celsius() {
    assert!(is_unit("celsius"));
    assert!(is_unit("C"));
}

#[test]
fn test_unit_alias_fahrenheit() {
    assert!(is_unit("fahrenheit"));
    assert!(is_unit("F"));
}

#[test]
fn test_unit_alias_kelvin() {
    assert!(is_unit("kelvin"));
    assert!(is_unit("K"));
}

#[test]
fn test_unit_alias_gram() {
    assert!(is_unit("gram"));
    assert!(is_unit("g"));
}

#[test]
fn test_unit_alias_pound() {
    assert!(is_unit("pound"));
    assert!(is_unit("lb"));
}

#[test]
fn test_unit_alias_ounce() {
    assert!(is_unit("ounce"));
    assert!(is_unit("oz"));
}

// ─── L14: inch canonical form ─────────────────────────────────────────

#[test]
fn test_inch_canonical_form() {
    let info = get_unit_info("inch").unwrap();
    assert_eq!(info.0, "inch");
    assert_eq!(info.1, "length");
}

#[test]
fn test_inch_abbreviation_resolves() {
    let info = get_unit_info("in").unwrap();
    assert_eq!(info.0, "inch");
    assert_eq!(info.1, "length");
}

// ─── UN-1/UN-2: μV/μA canonical forms ────────────────────────────────

#[test]
fn test_microvolt_canonical_uses_mu() {
    let info = get_unit_info("uV").unwrap();
    assert_eq!(
        info.0, "μV",
        "UN-1: microvolt canonical should use μ prefix, got {}",
        info.0
    );
}

#[test]
fn test_microvolt_mu_resolves() {
    let info = get_unit_info("μV").unwrap();
    assert_eq!(info.0, "μV", "UN-1: μV should resolve to itself");
}

#[test]
fn test_microamp_canonical_uses_mu() {
    let info = get_unit_info("uA").unwrap();
    assert_eq!(
        info.0, "μA",
        "UN-2: microamp canonical should use μ prefix, got {}",
        info.0
    );
}

#[test]
fn test_microamp_mu_resolves() {
    let info = get_unit_info("μA").unwrap();
    assert_eq!(info.0, "μA", "UN-2: μA should resolve to itself");
}
