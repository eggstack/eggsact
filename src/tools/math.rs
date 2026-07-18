use crate::calc::units::{
    convert_temperature, get_conversion_factor, get_unit_info, is_unit, PHYSICAL_CONSTANTS,
};
use crate::calc::{run, run_with_context, RunError};
use crate::mcp::budget::current_eval_context;
use crate::mcp::machine_codes;
use crate::mcp::response::ToolResponse;
use crate::tools::helpers::{
    _require_str, contains_true_division, json_type_name, MAX_EXPRESSION_LENGTH,
};
use serde_json::Value;

pub fn math_eval(args: &Value) -> ToolResponse {
    let expression = match _require_str(args, "expression", "math_eval") {
        Ok(s) => s,
        Err(e) => return *e,
    };

    if expression.chars().count() > MAX_EXPRESSION_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Expression length {} exceeds maximum {}",
                expression.chars().count(),
                MAX_EXPRESSION_LENGTH
            ),
            Some(vec!["Try a shorter expression".to_string()]),
            Some("math_eval"),
        );
    }

    let has_true_division = contains_true_division(expression);

    let expr_owned = expression.to_string();
    let current_ctx = current_eval_context().map(|ctx| ctx.clone());
    let eval_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(mut ctx) = current_ctx {
            run_with_context(&expr_owned, &mut ctx)
        } else {
            run(&expr_owned)
        }
    }))
    .unwrap_or_else(|_| {
        Err(crate::calc::RunError::Evaluation(
            "Expression evaluation panicked (possible stack overflow or resource limit)"
                .to_string(),
        ))
    });

    match eval_result {
        Ok((result, result_type)) => {
            let (value_str, unit, display) = if let Some(space_pos) = result.rfind(' ') {
                let numeric_part = &result[..space_pos];
                let unit_part = &result[space_pos + 1..];
                if numeric_part.parse::<f64>().is_ok() {
                    (
                        numeric_part.to_string(),
                        Some(unit_part.to_string()),
                        Some(result.clone()),
                    )
                } else {
                    (result.clone(), None, None)
                }
            } else {
                (result.clone(), None, None)
            };

            let obj = if let Some(ref u) = unit {
                let numeric_type = if let Ok(_int_val) = value_str.parse::<i64>() {
                    if value_str.contains('.') || value_str.contains('e') || value_str.contains('E')
                    {
                        "float"
                    } else {
                        "int"
                    }
                } else {
                    "float"
                };
                serde_json::json!({
                    "value": value_str,
                    "type": numeric_type,
                    "unit": u,
                    "display": display,
                })
            } else {
                let (out_value, out_type) = if has_true_division && result_type == "int" {
                    (format!("{}.0", value_str), "float".to_string())
                } else {
                    (value_str, result_type.to_string())
                };
                serde_json::json!({"value": out_value, "type": out_type})
            };
            ToolResponse::success(obj, Some("math_eval"))
        }
        Err(e) => {
            let (error_type, suggestions) = match &e {
                RunError::Evaluation(_) => (
                    "evaluation_error",
                    Some(vec!["Check expression syntax".to_string()]),
                ),
                RunError::Internal(_) => (
                    "internal_error",
                    Some(vec!["Check expression syntax".to_string()]),
                ),
            };
            ToolResponse::error_with_code(
                error_type,
                machine_codes::INVALID_ARGUMENTS,
                &e.to_string(),
                suggestions,
                Some("math_eval"),
            )
        }
    }
}

pub fn unit_convert(args: &Value) -> ToolResponse {
    if let Some(Value::Bool(_)) = args.get("value") {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "value must be a finite number, got {}",
                json_type_name(args.get("value").unwrap())
            ),
            None,
            Some("unit_convert"),
        );
    }
    let value = match args.get("value").and_then(|v| v.as_f64()) {
        Some(v) => v,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'value' parameter",
                None,
                Some("unit_convert"),
            )
        }
    };
    if !value.is_finite() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Value must be a finite number, got {}", value),
            None,
            Some("unit_convert"),
        );
    }
    let from_unit = match args.get("from_unit").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'from_unit' parameter",
                None,
                Some("unit_convert"),
            )
        }
    };
    let to_unit = match args.get("to_unit").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'to_unit' parameter",
                None,
                Some("unit_convert"),
            )
        }
    };

    if !is_unit(from_unit) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unknown unit: {}", from_unit),
            None,
            Some("unit_convert"),
        );
    }
    if !is_unit(to_unit) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unknown unit: {}", to_unit),
            None,
            Some("unit_convert"),
        );
    }

    let from_info = get_unit_info(from_unit);
    let to_info = get_unit_info(to_unit);

    let ((_, from_cat), (_, to_cat)) = match (&from_info, &to_info) {
        (Some(f), Some(t)) => (f, t),
        _ => {
            return ToolResponse::error_with_code(
                "conversion_error",
                machine_codes::INVALID_ARGUMENTS,
                &format!(
                    "Cannot determine category for unit(s): from='{}', to='{}'",
                    from_unit, to_unit
                ),
                None,
                Some("unit_convert"),
            );
        }
    };
    if from_cat != to_cat {
        return ToolResponse::error_with_code(
            "conversion_error",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "Cannot convert between incompatible categories: {} ({}) -> {} ({})",
                from_cat, from_unit, to_cat, to_unit
            ),
            None,
            Some("unit_convert"),
        );
    }

    if *from_cat == "temperature" && *to_cat == "temperature" {
        match convert_temperature(value, from_unit, to_unit) {
            Ok(result) => {
                if !result.is_finite() {
                    return ToolResponse::error_with_code(
                        "conversion_error",
                        machine_codes::INVALID_ARGUMENTS,
                        &format!("Conversion result is not finite: {}", result),
                        None,
                        Some("unit_convert"),
                    );
                }
                return ToolResponse::success(
                    serde_json::json!({
                        "value": result,
                        "from_unit": from_unit,
                        "to_unit": to_unit,
                        "factor": null,
                    }),
                    Some("unit_convert"),
                )
                .with_tool("unit_convert");
            }
            Err(e) => {
                return ToolResponse::error_with_code(
                    "conversion_error",
                    machine_codes::INVALID_ARGUMENTS,
                    &e,
                    None,
                    Some("unit_convert"),
                );
            }
        }
    }

    match get_conversion_factor(from_unit, to_unit) {
        Ok(factor) => {
            let result = value * factor;
            if !result.is_finite() {
                return ToolResponse::error_with_code(
                    "conversion_error",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("Conversion result is not finite: {}", result),
                    None,
                    Some("unit_convert"),
                );
            }
            ToolResponse::success(
                serde_json::json!({
                    "value": result,
                    "from_unit": from_unit,
                    "to_unit": to_unit,
                    "factor": factor,
                }),
                Some("unit_convert"),
            )
            .with_tool("unit_convert")
        }
        Err(e) => ToolResponse::error_with_code(
            "conversion_error",
            machine_codes::INVALID_ARGUMENTS,
            &e,
            None,
            Some("unit_convert"),
        ),
    }
}

pub fn unit_info(args: &Value) -> ToolResponse {
    let unit = match args.get("unit").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'unit' parameter",
                None,
                Some("unit_info"),
            )
        }
    };

    if let Some((canonical, category)) = get_unit_info(unit) {
        ToolResponse::success(
            serde_json::json!({
                "unit": unit,
                "canonical": canonical,
                "category": category,
                "is_valid": true,
            }),
            Some("unit_info"),
        )
        .with_tool("unit_info")
    } else {
        ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unknown unit: {}", unit),
            None,
            Some("unit_info"),
        )
    }
}

pub fn constant_lookup(args: &Value) -> ToolResponse {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'name' parameter",
                None,
                Some("constant_lookup"),
            )
        }
    };

    let constant = PHYSICAL_CONSTANTS.get(name.to_lowercase().as_str());
    if let Some(constant) = constant {
        ToolResponse::success(
            serde_json::json!({
                "name": name,
                "value": constant.value,
                "symbol": constant.symbol,
                "display_name": constant.display_name,
            }),
            Some("constant_lookup"),
        )
        .with_tool("constant_lookup")
    } else {
        ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unknown constant: {}", name),
            None,
            Some("constant_lookup"),
        )
    }
}
