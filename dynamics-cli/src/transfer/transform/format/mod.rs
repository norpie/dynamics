//! Format transform - string interpolation with expressions
//!
//! Supports:
//! - Field interpolation: `${field}`, `${field.lookup}`
//! - Math operations: `${a + b}`, `${a * b}`
//! - Comparisons: `${a == b}`, `${a < b}`
//! - Ternary conditionals: `${cond ? then : else}`
//! - Null coalesce: `${a ?? b ?? 'default'}`
//! - Format specifiers: `${price:,.2f}`

mod ast;
mod eval;
mod parser;

pub use ast::{
    CompareOp, FormatExpr, FormatPart, FormatSpec, FormatTemplate, FormatType, MathOp, NullHandling,
};
pub use eval::evaluate;
pub use parser::{ParseError, parse_template};
