//! Evaluator for format templates

use super::ast::*;
use crate::transfer::FieldPath;
use crate::transfer::Value;
use crate::transfer::transform::resolve_path;

/// Evaluate a format template against a record
pub fn evaluate(
    template: &FormatTemplate,
    record: &serde_json::Value,
    null_handling: NullHandling,
) -> Result<String, String> {
    let mut result = String::new();

    for part in &template.parts {
        match part {
            FormatPart::Literal(s) => result.push_str(s),
            FormatPart::Expr(expr) => {
                let value = eval_expr(expr, record, null_handling)?;
                result.push_str(&value_to_string(&value, None));
            }
        }
    }

    Ok(result)
}

/// Evaluate an expression and return a Value
fn eval_expr(
    expr: &FormatExpr,
    record: &serde_json::Value,
    null_handling: NullHandling,
) -> Result<Value, String> {
    match expr {
        FormatExpr::Field(path) => Ok(resolve_path(record, path)),

        FormatExpr::Constant(value) => Ok(value.clone()),

        FormatExpr::Math { left, op, right } => {
            let left_val = eval_expr(left, record, null_handling)?;
            let right_val = eval_expr(right, record, null_handling)?;
            eval_math(&left_val, *op, &right_val, null_handling)
        }

        FormatExpr::Compare { left, op, right } => {
            let left_val = eval_expr(left, record, null_handling)?;
            let right_val = eval_expr(right, record, null_handling)?;
            let result = eval_compare(&left_val, *op, &right_val, null_handling)?;
            Ok(Value::Bool(result))
        }

        FormatExpr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            let cond_val = eval_expr(condition, record, null_handling)?;
            if is_truthy(&cond_val) {
                eval_expr(then_expr, record, null_handling)
            } else {
                eval_expr(else_expr, record, null_handling)
            }
        }

        FormatExpr::Coalesce { exprs } => {
            for e in exprs {
                let val = eval_expr(e, record, null_handling)?;
                if !val.is_null() {
                    return Ok(val);
                }
            }
            // All values were null
            Ok(Value::Null)
        }

        FormatExpr::Formatted { expr, spec } => {
            let val = eval_expr(expr, record, null_handling)?;
            let formatted = apply_format_spec(&val, spec)?;
            Ok(Value::String(formatted))
        }

        FormatExpr::Negate(inner) => {
            let val = eval_expr(inner, record, null_handling)?;
            eval_negate(&val, null_handling)
        }
    }
}

/// Evaluate a math operation
fn eval_math(
    left: &Value,
    op: MathOp,
    right: &Value,
    null_handling: NullHandling,
) -> Result<Value, String> {
    // Handle null
    if left.is_null() || right.is_null() {
        return match null_handling {
            NullHandling::Error => Err("null value in math operation".to_string()),
            NullHandling::Zero => {
                let left = if left.is_null() { &Value::Int(0) } else { left };
                let right = if right.is_null() {
                    &Value::Int(0)
                } else {
                    right
                };
                eval_math(left, op, right, NullHandling::Error)
            }
            NullHandling::Empty => Ok(Value::Null),
        };
    }

    // Type checking - only numbers allowed
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => {
            let result = match op {
                MathOp::Add => a.checked_add(*b).ok_or("integer overflow")?,
                MathOp::Sub => a.checked_sub(*b).ok_or("integer underflow")?,
                MathOp::Mul => a.checked_mul(*b).ok_or("integer overflow")?,
                MathOp::Div => {
                    if *b == 0 {
                        return Err("division by zero".to_string());
                    }
                    a / b
                }
            };
            Ok(Value::Int(result))
        }
        (Value::Float(a), Value::Float(b)) => {
            let result = match op {
                MathOp::Add => a + b,
                MathOp::Sub => a - b,
                MathOp::Mul => a * b,
                MathOp::Div => {
                    if *b == 0.0 {
                        return Err("division by zero".to_string());
                    }
                    a / b
                }
            };
            Ok(Value::Float(result))
        }
        (Value::Int(a), Value::Float(b)) => {
            let a = *a as f64;
            let result = match op {
                MathOp::Add => a + b,
                MathOp::Sub => a - b,
                MathOp::Mul => a * b,
                MathOp::Div => {
                    if *b == 0.0 {
                        return Err("division by zero".to_string());
                    }
                    a / b
                }
            };
            Ok(Value::Float(result))
        }
        (Value::Float(a), Value::Int(b)) => {
            let b = *b as f64;
            let result = match op {
                MathOp::Add => a + b,
                MathOp::Sub => a - b,
                MathOp::Mul => a * b,
                MathOp::Div => {
                    if b == 0.0 {
                        return Err("division by zero".to_string());
                    }
                    a / b
                }
            };
            Ok(Value::Float(result))
        }
        (Value::Bool(_), _) | (_, Value::Bool(_)) => {
            Err("boolean values cannot be used in math operations".to_string())
        }
        (Value::String(_), _) | (_, Value::String(_)) => Err(
            "string values cannot be used in math operations (use multiple ${} for concatenation)"
                .to_string(),
        ),
        _ => Err(format!(
            "cannot perform {} on {} and {}",
            op,
            type_name(left),
            type_name(right)
        )),
    }
}

/// Evaluate a comparison operation
fn eval_compare(
    left: &Value,
    op: CompareOp,
    right: &Value,
    null_handling: NullHandling,
) -> Result<bool, String> {
    // Null comparisons
    if left.is_null() || right.is_null() {
        return match null_handling {
            NullHandling::Error => Err("null value in comparison".to_string()),
            NullHandling::Zero | NullHandling::Empty => {
                // null == null is true, null == anything else is false
                Ok(
                    matches!(op, CompareOp::Eq) && left.is_null() && right.is_null()
                        || matches!(op, CompareOp::Ne) && !(left.is_null() && right.is_null()),
                )
            }
        };
    }

    // Equality/inequality works across types
    match op {
        CompareOp::Eq => Ok(values_equal(left, right)),
        CompareOp::Ne => Ok(!values_equal(left, right)),
        CompareOp::Lt | CompareOp::Le | CompareOp::Gt | CompareOp::Ge => {
            // Ordering only works for same types
            compare_ordered(left, op, right)
        }
    }
}

/// Check if two values are equal (with type coercion for numbers)
fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        // Same types
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Null, Value::Null) => true,
        (Value::Guid(a), Value::Guid(b)) => a == b,
        (Value::DateTime(a), Value::DateTime(b)) => a == b,
        (Value::OptionSet(a), Value::OptionSet(b)) => a == b,

        // Int/Float coercion
        (Value::Int(a), Value::Float(b)) => (*a as f64 - b).abs() < f64::EPSILON,
        (Value::Float(a), Value::Int(b)) => (a - *b as f64).abs() < f64::EPSILON,

        // Int/OptionSet coercion (OptionSet is stored as i32)
        (Value::Int(a), Value::OptionSet(b)) => *a == *b as i64,
        (Value::OptionSet(a), Value::Int(b)) => *a as i64 == *b,

        // Different types are not equal
        _ => false,
    }
}

/// Compare values with ordering operators
fn compare_ordered(left: &Value, op: CompareOp, right: &Value) -> Result<bool, String> {
    let ordering = match (left, right) {
        (Value::Int(a), Value::Int(b)) => a.cmp(b),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Int(a), Value::Float(b)) => (*a as f64)
            .partial_cmp(b)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::Float(a), Value::Int(b)) => a
            .partial_cmp(&(*b as f64))
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::DateTime(a), Value::DateTime(b)) => a.cmp(b),
        _ => {
            return Err(format!(
                "cannot compare {} and {} with {}",
                type_name(left),
                type_name(right),
                op
            ));
        }
    };

    Ok(match op {
        CompareOp::Lt => ordering == std::cmp::Ordering::Less,
        CompareOp::Le => ordering != std::cmp::Ordering::Greater,
        CompareOp::Gt => ordering == std::cmp::Ordering::Greater,
        CompareOp::Ge => ordering != std::cmp::Ordering::Less,
        _ => unreachable!(),
    })
}

/// Negate a value
fn eval_negate(val: &Value, null_handling: NullHandling) -> Result<Value, String> {
    if val.is_null() {
        return match null_handling {
            NullHandling::Error => Err("null value in negation".to_string()),
            NullHandling::Zero => Ok(Value::Int(0)),
            NullHandling::Empty => Ok(Value::Null),
        };
    }

    match val {
        Value::Int(n) => Ok(Value::Int(-n)),
        Value::Float(n) => Ok(Value::Float(-n)),
        _ => Err(format!("cannot negate {}", type_name(val))),
    }
}

/// JavaScript-style truthiness
fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Float(n) => *n != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::OptionSet(n) => *n != 0,
        // GUIDs, DateTimes are always truthy
        _ => true,
    }
}

/// Convert a value to string for output
fn value_to_string(val: &Value, _spec: Option<&FormatSpec>) -> String {
    match val {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Float(n) => {
            // Default float formatting
            if n.fract() == 0.0 {
                format!("{:.0}", n)
            } else {
                n.to_string()
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Guid(g) => g.to_string(),
        Value::DateTime(dt) => dt.to_rfc3339(),
        Value::OptionSet(n) => n.to_string(),
        Value::Dynamic(_) => "[dynamic]".to_string(),
    }
}

/// Apply a format specifier to a value
fn apply_format_spec(val: &Value, spec: &FormatSpec) -> Result<String, String> {
    if val.is_null() {
        return Ok(String::new());
    }

    match spec.format_type {
        FormatType::Auto => {
            // Auto-detect based on value type and spec
            match val {
                Value::Float(n) => format_float(*n, spec),
                Value::Int(n) => format_int(*n, spec),
                Value::DateTime(dt) => Ok(dt.to_rfc3339()),
                _ => Ok(value_to_string(val, Some(spec))),
            }
        }
        FormatType::Float => {
            let n = to_float(val)?;
            format_float(n, spec)
        }
        FormatType::Integer => {
            let n = to_int(val)?;
            format_int(n, spec)
        }
        FormatType::Date => match val {
            Value::DateTime(dt) => Ok(dt.format("%Y-%m-%d").to_string()),
            _ => Err(format!("cannot format {} as date", type_name(val))),
        },
        FormatType::DateTime => match val {
            Value::DateTime(dt) => Ok(dt.to_rfc3339()),
            _ => Err(format!("cannot format {} as datetime", type_name(val))),
        },
        FormatType::Percent => {
            let n = to_float(val)?;
            let pct = n * 100.0;
            let precision = spec.precision.unwrap_or(0) as usize;
            let formatted = format!("{:.prec$}%", pct, prec = precision);
            if spec.thousands_sep {
                Ok(add_thousands_sep(&formatted.replace('%', "")) + "%")
            } else {
                Ok(formatted)
            }
        }
    }
}

/// Format a float with the given spec
fn format_float(n: f64, spec: &FormatSpec) -> Result<String, String> {
    let precision = spec.precision.unwrap_or(2) as usize;
    let formatted = format!("{:.prec$}", n, prec = precision);

    if spec.thousands_sep {
        Ok(add_thousands_sep(&formatted))
    } else {
        Ok(formatted)
    }
}

/// Format an integer with the given spec
fn format_int(n: i64, spec: &FormatSpec) -> Result<String, String> {
    let formatted = n.to_string();

    if spec.thousands_sep {
        Ok(add_thousands_sep(&formatted))
    } else {
        Ok(formatted)
    }
}

/// Add thousands separator to a number string
fn add_thousands_sep(s: &str) -> String {
    // Split on decimal point
    let (int_part, dec_part) = if let Some(pos) = s.find('.') {
        (&s[..pos], Some(&s[pos..]))
    } else {
        (s, None)
    };

    // Handle negative
    let (sign, int_part) = if int_part.starts_with('-') {
        ("-", &int_part[1..])
    } else {
        ("", int_part)
    };

    // Add commas every 3 digits from the right
    let mut result = String::new();
    for (i, c) in int_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    let int_with_sep: String = result.chars().rev().collect();

    format!("{}{}{}", sign, int_with_sep, dec_part.unwrap_or(""))
}

/// Convert a value to float
fn to_float(val: &Value) -> Result<f64, String> {
    match val {
        Value::Float(n) => Ok(*n),
        Value::Int(n) => Ok(*n as f64),
        Value::OptionSet(n) => Ok(*n as f64),
        _ => Err(format!("cannot convert {} to float", type_name(val))),
    }
}

/// Convert a value to int
fn to_int(val: &Value) -> Result<i64, String> {
    match val {
        Value::Int(n) => Ok(*n),
        Value::Float(n) => Ok(*n as i64),
        Value::OptionSet(n) => Ok(*n as i64),
        _ => Err(format!("cannot convert {} to integer", type_name(val))),
    }
}

/// Get a type name for error messages
fn type_name(val: &Value) -> &'static str {
    match val {
        Value::Null => "null",
        Value::String(_) => "string",
        Value::Int(_) => "integer",
        Value::Float(_) => "float",
        Value::Bool(_) => "boolean",
        Value::Guid(_) => "guid",
        Value::DateTime(_) => "datetime",
        Value::OptionSet(_) => "optionset",
        Value::Dynamic(_) => "dynamic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::transform::format::parse_template;
    use serde_json::json;

    fn eval_template(template: &str, record: serde_json::Value) -> Result<String, String> {
        let parsed = parse_template(template).map_err(|e| e.to_string())?;
        evaluate(&parsed, &record, NullHandling::Error)
    }

    #[test]
    fn test_eval_literal() {
        let result = eval_template("Hello, World!", json!({})).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_eval_simple_field() {
        let result = eval_template("${name}", json!({"name": "John"})).unwrap();
        assert_eq!(result, "John");
    }

    #[test]
    fn test_eval_lookup_field() {
        let result = eval_template(
            "${accountid.name}",
            json!({"accountid": {"name": "Contoso"}}),
        )
        .unwrap();
        assert_eq!(result, "Contoso");
    }

    #[test]
    fn test_eval_mixed() {
        let result = eval_template(
            "Name: ${name}, Age: ${age}",
            json!({"name": "John", "age": 30}),
        )
        .unwrap();
        assert_eq!(result, "Name: John, Age: 30");
    }

    #[test]
    fn test_eval_math_add() {
        let result = eval_template("${a + b}", json!({"a": 10, "b": 5})).unwrap();
        assert_eq!(result, "15");
    }

    #[test]
    fn test_eval_math_multiply() {
        let result =
            eval_template("${quantity * price}", json!({"quantity": 3, "price": 10.5})).unwrap();
        assert_eq!(result, "31.5");
    }

    #[test]
    fn test_eval_math_precedence() {
        let result = eval_template("${a + b * c}", json!({"a": 1, "b": 2, "c": 3})).unwrap();
        assert_eq!(result, "7"); // 1 + (2 * 3) = 7
    }

    #[test]
    fn test_eval_comparison() {
        let result = eval_template("${a == b}", json!({"a": 5, "b": 5})).unwrap();
        assert_eq!(result, "true");

        let result = eval_template("${a < b}", json!({"a": 3, "b": 5})).unwrap();
        assert_eq!(result, "true");
    }

    #[test]
    fn test_eval_ternary() {
        let result = eval_template("${active ? 'Yes' : 'No'}", json!({"active": true})).unwrap();
        assert_eq!(result, "Yes");

        let result = eval_template("${active ? 'Yes' : 'No'}", json!({"active": false})).unwrap();
        assert_eq!(result, "No");
    }

    #[test]
    fn test_eval_ternary_with_comparison() {
        let result = eval_template(
            "${status == 0 ? 'Active' : 'Inactive'}",
            json!({"status": 0}),
        )
        .unwrap();
        assert_eq!(result, "Active");

        let result = eval_template(
            "${status == 0 ? 'Active' : 'Inactive'}",
            json!({"status": 1}),
        )
        .unwrap();
        assert_eq!(result, "Inactive");
    }

    #[test]
    fn test_eval_coalesce() {
        let result = eval_template("${name ?? 'Unknown'}", json!({"name": null})).unwrap();
        assert_eq!(result, "Unknown");

        let result = eval_template("${name ?? 'Unknown'}", json!({"name": "John"})).unwrap();
        assert_eq!(result, "John");
    }

    #[test]
    fn test_eval_chained_coalesce() {
        let result = eval_template(
            "${primary ?? secondary ?? 'None'}",
            json!({"primary": null, "secondary": "Backup"}),
        )
        .unwrap();
        assert_eq!(result, "Backup");
    }

    #[test]
    fn test_eval_format_spec_precision() {
        let result = eval_template("${price:.2f}", json!({"price": 10.5})).unwrap();
        assert_eq!(result, "10.50");
    }

    #[test]
    fn test_eval_format_spec_thousands() {
        let result = eval_template("${amount:,d}", json!({"amount": 1234567})).unwrap();
        assert_eq!(result, "1,234,567");
    }

    #[test]
    fn test_eval_format_spec_combined() {
        let result = eval_template("${price:,.2f}", json!({"price": 1234567.89})).unwrap();
        assert_eq!(result, "1,234,567.89");
    }

    #[test]
    fn test_eval_negation() {
        let result = eval_template("${-price}", json!({"price": 100})).unwrap();
        assert_eq!(result, "-100");
    }

    #[test]
    fn test_eval_null_in_math_error() {
        let result = eval_template("${a + b}", json!({"a": 10, "b": null}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("null"));
    }

    #[test]
    fn test_eval_null_in_math_zero_mode() {
        let parsed = parse_template("${a + b}").unwrap();
        let result = evaluate(&parsed, &json!({"a": 10, "b": null}), NullHandling::Zero).unwrap();
        assert_eq!(result, "10");
    }

    #[test]
    fn test_eval_string_in_math_error() {
        let result = eval_template("${a + b}", json!({"a": "hello", "b": 5}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("string"));
    }

    #[test]
    fn test_eval_boolean_in_math_error() {
        let result = eval_template("${a + b}", json!({"a": true, "b": 1}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("boolean"));
    }

    #[test]
    fn test_eval_truthiness() {
        // 0 is falsy
        let result =
            eval_template("${count ? 'Has items' : 'Empty'}", json!({"count": 0})).unwrap();
        assert_eq!(result, "Empty");

        // Non-zero is truthy
        let result =
            eval_template("${count ? 'Has items' : 'Empty'}", json!({"count": 5})).unwrap();
        assert_eq!(result, "Has items");

        // Empty string is falsy
        let result = eval_template("${name ? 'Named' : 'Anonymous'}", json!({"name": ""})).unwrap();
        assert_eq!(result, "Anonymous");
    }

    #[test]
    fn test_eval_complex_expression() {
        let result = eval_template(
            "Total: ${(quantity * price):,.2f}",
            json!({"quantity": 1000, "price": 49.99}),
        )
        .unwrap();
        assert_eq!(result, "Total: 49,990.00");
    }

    #[test]
    fn test_add_thousands_sep() {
        assert_eq!(add_thousands_sep("1234567"), "1,234,567");
        assert_eq!(add_thousands_sep("123"), "123");
        assert_eq!(add_thousands_sep("-1234567"), "-1,234,567");
        assert_eq!(add_thousands_sep("1234567.89"), "1,234,567.89");
    }
}
