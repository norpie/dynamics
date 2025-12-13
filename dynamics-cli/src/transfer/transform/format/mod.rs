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
mod parser;
mod eval;

pub use ast::{
    FormatTemplate, FormatPart, FormatExpr,
    MathOp, CompareOp, FormatSpec, FormatType,
    NullHandling,
};
pub use parser::{parse_template, ParseError};
pub use eval::evaluate;
