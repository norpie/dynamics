//! AST types for format templates

use serde::{Deserialize, Serialize};

use crate::transfer::FieldPath;
use crate::transfer::Value;

/// A parsed format template containing literal text and expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatTemplate {
    /// The parts of the template (literals and expressions)
    pub parts: Vec<FormatPart>,
    /// The original template string (for display/debugging)
    #[serde(default)]
    pub source: String,
}

impl FormatTemplate {
    /// Create a new format template
    pub fn new(parts: Vec<FormatPart>, source: String) -> Self {
        Self { parts, source }
    }

    /// Create a template with just a literal string (no expressions)
    pub fn literal(s: impl Into<String>) -> Self {
        let s = s.into();
        Self {
            parts: vec![FormatPart::Literal(s.clone())],
            source: s,
        }
    }

    /// Get all field paths referenced in this template
    pub fn field_paths(&self) -> Vec<&FieldPath> {
        let mut paths = Vec::new();
        for part in &self.parts {
            if let FormatPart::Expr(expr) = part {
                collect_field_paths(expr, &mut paths);
            }
        }
        paths
    }

    /// Get all base field names (for $select)
    pub fn base_fields(&self) -> Vec<&str> {
        self.field_paths()
            .iter()
            .map(|p| p.base_field())
            .collect()
    }

    /// Get expand specs for lookup traversals
    pub fn expand_specs(&self) -> Vec<(&str, &str)> {
        self.field_paths()
            .iter()
            .filter_map(|p| p.lookup_field().map(|lf| (p.base_field(), lf)))
            .collect()
    }
}

impl std::fmt::Display for FormatTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)
    }
}

/// Collect all field paths from an expression recursively
fn collect_field_paths<'a>(expr: &'a FormatExpr, paths: &mut Vec<&'a FieldPath>) {
    match expr {
        FormatExpr::Field(path) => paths.push(path),
        FormatExpr::Constant(_) => {}
        FormatExpr::Math { left, right, .. } => {
            collect_field_paths(left, paths);
            collect_field_paths(right, paths);
        }
        FormatExpr::Compare { left, right, .. } => {
            collect_field_paths(left, paths);
            collect_field_paths(right, paths);
        }
        FormatExpr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_field_paths(condition, paths);
            collect_field_paths(then_expr, paths);
            collect_field_paths(else_expr, paths);
        }
        FormatExpr::Coalesce { exprs } => {
            for e in exprs {
                collect_field_paths(e, paths);
            }
        }
        FormatExpr::Formatted { expr, .. } => {
            collect_field_paths(expr, paths);
        }
        FormatExpr::Negate(inner) => {
            collect_field_paths(inner, paths);
        }
    }
}

/// A part of a format template
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FormatPart {
    /// Literal text (not an expression)
    Literal(String),
    /// An expression to evaluate: `${...}`
    Expr(FormatExpr),
}

/// An expression within a format template
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FormatExpr {
    /// A field reference: `field` or `field.lookup`
    Field(FieldPath),
    /// A constant value: `'string'`, `123`, `true`
    Constant(Value),
    /// Math operation: `a + b`, `a * b`
    Math {
        left: Box<FormatExpr>,
        op: MathOp,
        right: Box<FormatExpr>,
    },
    /// Comparison: `a == b`, `a < b`
    Compare {
        left: Box<FormatExpr>,
        op: CompareOp,
        right: Box<FormatExpr>,
    },
    /// Ternary conditional: `cond ? then : else`
    Ternary {
        condition: Box<FormatExpr>,
        then_expr: Box<FormatExpr>,
        else_expr: Box<FormatExpr>,
    },
    /// Null coalesce: `a ?? b ?? c`
    Coalesce {
        exprs: Vec<FormatExpr>,
    },
    /// Formatted expression with format spec: `expr:,.2f`
    Formatted {
        expr: Box<FormatExpr>,
        spec: FormatSpec,
    },
    /// Negation: `-expr`
    Negate(Box<FormatExpr>),
}

/// Math operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl std::fmt::Display for MathOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MathOp::Add => write!(f, "+"),
            MathOp::Sub => write!(f, "-"),
            MathOp::Mul => write!(f, "*"),
            MathOp::Div => write!(f, "/"),
        }
    }
}

/// Comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl std::fmt::Display for CompareOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompareOp::Eq => write!(f, "=="),
            CompareOp::Ne => write!(f, "!="),
            CompareOp::Lt => write!(f, "<"),
            CompareOp::Le => write!(f, "<="),
            CompareOp::Gt => write!(f, ">"),
            CompareOp::Ge => write!(f, ">="),
        }
    }
}

/// Format specifier for controlling output formatting
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatSpec {
    /// Use thousands separator (`,`)
    pub thousands_sep: bool,
    /// Decimal precision (`.2` means 2 decimal places)
    pub precision: Option<u8>,
    /// Format type
    pub format_type: FormatType,
}

impl Default for FormatSpec {
    fn default() -> Self {
        Self {
            thousands_sep: false,
            precision: None,
            format_type: FormatType::Auto,
        }
    }
}

impl std::fmt::Display for FormatSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.thousands_sep {
            write!(f, ",")?;
        }
        if let Some(prec) = self.precision {
            write!(f, ".{}", prec)?;
        }
        match self.format_type {
            FormatType::Auto => {}
            FormatType::Float => write!(f, "f")?,
            FormatType::Integer => write!(f, "d")?,
            FormatType::Date => write!(f, "date")?,
            FormatType::DateTime => write!(f, "datetime")?,
            FormatType::Percent => write!(f, "%")?,
        }
        Ok(())
    }
}

/// Format type specifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FormatType {
    /// Infer from value type
    #[default]
    Auto,
    /// Decimal float
    Float,
    /// Integer
    Integer,
    /// Date only (YYYY-MM-DD)
    Date,
    /// Full datetime
    DateTime,
    /// Percentage (multiply by 100, add %)
    Percent,
}

/// How to handle null values in expressions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NullHandling {
    /// Error when null is encountered in math/comparison
    #[default]
    Error,
    /// Treat null as empty string in output
    Empty,
    /// Treat null as 0 in math operations
    Zero,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_field_paths() {
        let template = FormatTemplate::new(
            vec![
                FormatPart::Literal("Name: ".to_string()),
                FormatPart::Expr(FormatExpr::Field(FieldPath::simple("name"))),
                FormatPart::Literal(", Company: ".to_string()),
                FormatPart::Expr(FormatExpr::Field(FieldPath::lookup("accountid", "name"))),
            ],
            "${name} ${accountid.name}".to_string(),
        );

        let paths = template.field_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].base_field(), "name");
        assert_eq!(paths[1].base_field(), "accountid");
        assert_eq!(paths[1].lookup_field(), Some("name"));
    }

    #[test]
    fn test_template_expand_specs() {
        let template = FormatTemplate::new(
            vec![
                FormatPart::Expr(FormatExpr::Field(FieldPath::simple("name"))),
                FormatPart::Expr(FormatExpr::Field(FieldPath::lookup("accountid", "name"))),
                FormatPart::Expr(FormatExpr::Field(FieldPath::lookup("parentid", "revenue"))),
            ],
            "".to_string(),
        );

        let specs = template.expand_specs();
        assert_eq!(specs.len(), 2);
        assert!(specs.contains(&("accountid", "name")));
        assert!(specs.contains(&("parentid", "revenue")));
    }

    #[test]
    fn test_format_spec_display() {
        let spec = FormatSpec {
            thousands_sep: true,
            precision: Some(2),
            format_type: FormatType::Float,
        };
        assert_eq!(spec.to_string(), ",.2f");

        let spec2 = FormatSpec {
            thousands_sep: false,
            precision: Some(0),
            format_type: FormatType::Percent,
        };
        assert_eq!(spec2.to_string(), ".0%");
    }
}
