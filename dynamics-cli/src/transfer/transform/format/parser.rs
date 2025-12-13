//! Recursive descent parser for format templates

use super::ast::*;
use crate::transfer::FieldPath;
use crate::transfer::Value;

/// Parse error with position information
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
    pub context: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at position {}: {}", self.position, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse a format template string into a FormatTemplate AST
pub fn parse_template(input: &str) -> Result<FormatTemplate, ParseError> {
    let mut parts = Vec::new();
    let mut current_literal = String::new();
    let mut chars = input.char_indices().peekable();

    while let Some((pos, ch)) = chars.next() {
        if ch == '$' {
            if let Some(&(_, '{')) = chars.peek() {
                // Start of expression
                chars.next(); // consume '{'

                // Save any accumulated literal
                if !current_literal.is_empty() {
                    parts.push(FormatPart::Literal(std::mem::take(&mut current_literal)));
                }

                // Find the matching closing brace
                let expr_start = pos + 2;
                let mut brace_depth = 1;
                let mut expr_end = expr_start;

                for (i, c) in chars.by_ref() {
                    if c == '{' {
                        brace_depth += 1;
                    } else if c == '}' {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            expr_end = i;
                            break;
                        }
                    }
                }

                if brace_depth != 0 {
                    return Err(ParseError {
                        message: "unclosed expression, expected '}'".to_string(),
                        position: pos,
                        context: input[pos..].chars().take(20).collect(),
                    });
                }

                // Parse the expression
                let expr_str = &input[expr_start..expr_end];
                let expr = parse_expression(expr_str, expr_start)?;
                parts.push(FormatPart::Expr(expr));
            } else {
                // Just a literal '$'
                current_literal.push(ch);
            }
        } else {
            current_literal.push(ch);
        }
    }

    // Don't forget trailing literal
    if !current_literal.is_empty() {
        parts.push(FormatPart::Literal(current_literal));
    }

    Ok(FormatTemplate::new(parts, input.to_string()))
}

/// Token types for the expression lexer
#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Literals
    Ident(String),
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Question,
    Colon,
    DoubleQuestion, // ??
    Dot,
    Comma,
    LParen,
    RParen,

    // End of input
    Eof,
}

/// Lexer for expression tokens
struct Lexer<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    base_pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str, base_pos: usize) -> Self {
        Self {
            input,
            chars: input.char_indices().peekable(),
            base_pos,
        }
    }

    fn current_pos(&mut self) -> usize {
        self.chars
            .peek()
            .map(|(i, _)| *i)
            .unwrap_or(self.input.len())
            + self.base_pos
    }

    fn skip_whitespace(&mut self) {
        while let Some(&(_, ch)) = self.chars.peek() {
            if ch.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();

        let Some(&(pos, ch)) = self.chars.peek() else {
            return Ok(Token::Eof);
        };

        // Single-char tokens
        match ch {
            '+' => {
                self.chars.next();
                return Ok(Token::Plus);
            }
            '-' => {
                self.chars.next();
                return Ok(Token::Minus);
            }
            '*' => {
                self.chars.next();
                return Ok(Token::Star);
            }
            '/' => {
                self.chars.next();
                return Ok(Token::Slash);
            }
            '.' => {
                self.chars.next();
                return Ok(Token::Dot);
            }
            ',' => {
                self.chars.next();
                return Ok(Token::Comma);
            }
            '(' => {
                self.chars.next();
                return Ok(Token::LParen);
            }
            ')' => {
                self.chars.next();
                return Ok(Token::RParen);
            }
            ':' => {
                self.chars.next();
                return Ok(Token::Colon);
            }
            _ => {}
        }

        // Two-char tokens
        if ch == '?' {
            self.chars.next();
            if let Some(&(_, '?')) = self.chars.peek() {
                self.chars.next();
                return Ok(Token::DoubleQuestion);
            }
            return Ok(Token::Question);
        }

        if ch == '=' {
            self.chars.next();
            if let Some(&(_, '=')) = self.chars.peek() {
                self.chars.next();
                return Ok(Token::Eq);
            }
            return Err(ParseError {
                message: "expected '==' for equality comparison".to_string(),
                position: pos + self.base_pos,
                context: self.input[pos..].chars().take(10).collect(),
            });
        }

        if ch == '!' {
            self.chars.next();
            if let Some(&(_, '=')) = self.chars.peek() {
                self.chars.next();
                return Ok(Token::Ne);
            }
            return Err(ParseError {
                message: "expected '!=' for not-equal comparison".to_string(),
                position: pos + self.base_pos,
                context: self.input[pos..].chars().take(10).collect(),
            });
        }

        if ch == '<' {
            self.chars.next();
            if let Some(&(_, '=')) = self.chars.peek() {
                self.chars.next();
                return Ok(Token::Le);
            }
            return Ok(Token::Lt);
        }

        if ch == '>' {
            self.chars.next();
            if let Some(&(_, '=')) = self.chars.peek() {
                self.chars.next();
                return Ok(Token::Ge);
            }
            return Ok(Token::Gt);
        }

        // String literal
        if ch == '\'' {
            self.chars.next(); // consume opening quote
            let mut s = String::new();
            loop {
                match self.chars.next() {
                    Some((_, '\'')) => break,
                    Some((_, c)) => s.push(c),
                    None => {
                        return Err(ParseError {
                            message: "unclosed string literal".to_string(),
                            position: pos + self.base_pos,
                            context: self.input[pos..].chars().take(20).collect(),
                        })
                    }
                }
            }
            return Ok(Token::String(s));
        }

        // Number
        if ch.is_ascii_digit() {
            let start = pos;
            let mut has_dot = false;

            while let Some(&(_, c)) = self.chars.peek() {
                if c.is_ascii_digit() {
                    self.chars.next();
                } else if c == '.' && !has_dot {
                    // Look ahead to see if this is a decimal point or field separator
                    let mut peek_chars = self.input[self.chars.peek().map(|(i, _)| *i).unwrap_or(0)..]
                        .chars()
                        .skip(1);
                    if peek_chars.next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        has_dot = true;
                        self.chars.next();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            let end = self.chars.peek().map(|(i, _)| *i).unwrap_or(self.input.len());
            let num_str = &self.input[start..end];

            if has_dot {
                let n: f64 = num_str.parse().map_err(|_| ParseError {
                    message: format!("invalid float: {}", num_str),
                    position: start + self.base_pos,
                    context: num_str.to_string(),
                })?;
                return Ok(Token::Float(n));
            } else {
                let n: i64 = num_str.parse().map_err(|_| ParseError {
                    message: format!("invalid integer: {}", num_str),
                    position: start + self.base_pos,
                    context: num_str.to_string(),
                })?;
                return Ok(Token::Int(n));
            }
        }

        // Identifier or keyword
        if ch.is_alphabetic() || ch == '_' {
            let start = pos;
            while let Some(&(_, c)) = self.chars.peek() {
                if c.is_alphanumeric() || c == '_' {
                    self.chars.next();
                } else {
                    break;
                }
            }
            let end = self.chars.peek().map(|(i, _)| *i).unwrap_or(self.input.len());
            let ident = &self.input[start..end];

            // Check for keywords
            match ident {
                "true" => return Ok(Token::Bool(true)),
                "false" => return Ok(Token::Bool(false)),
                _ => return Ok(Token::Ident(ident.to_string())),
            }
        }

        Err(ParseError {
            message: format!("unexpected character: '{}'", ch),
            position: pos + self.base_pos,
            context: self.input[pos..].chars().take(10).collect(),
        })
    }

    fn peek_token(&mut self) -> Result<Token, ParseError> {
        let saved_chars = self.chars.clone();
        let tok = self.next_token()?;
        self.chars = saved_chars;
        Ok(tok)
    }
}

/// Expression parser
struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, base_pos: usize) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(input, base_pos);
        let current = lexer.next_token()?;
        Ok(Self { lexer, current })
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        if self.current == expected {
            self.advance()
        } else {
            Err(ParseError {
                message: format!("expected {:?}, found {:?}", expected, self.current),
                position: self.lexer.current_pos(),
                context: String::new(),
            })
        }
    }

    /// Parse the full expression
    fn parse(&mut self) -> Result<FormatExpr, ParseError> {
        let expr = self.parse_ternary()?;

        // Format specifier is only allowed at the top level (not inside ternary branches)
        let expr = if self.current == Token::Colon {
            self.advance()?;
            let spec = self.parse_format_spec()?;
            FormatExpr::Formatted {
                expr: Box::new(expr),
                spec,
            }
        } else {
            expr
        };

        if self.current != Token::Eof {
            return Err(ParseError {
                message: format!("unexpected token after expression: {:?}", self.current),
                position: self.lexer.current_pos(),
                context: String::new(),
            });
        }

        Ok(expr)
    }

    /// ternary = coalesce ("?" expression ":" expression)?
    fn parse_ternary(&mut self) -> Result<FormatExpr, ParseError> {
        let condition = self.parse_coalesce()?;

        if self.current == Token::Question {
            self.advance()?;
            let then_expr = self.parse_ternary()?; // Right-associative
            self.expect(Token::Colon)?;
            let else_expr = self.parse_ternary()?;

            return Ok(FormatExpr::Ternary {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            });
        }

        Ok(condition)
    }

    /// coalesce = comparison ("??" comparison)*
    fn parse_coalesce(&mut self) -> Result<FormatExpr, ParseError> {
        let mut expr = self.parse_comparison()?;

        if self.current == Token::DoubleQuestion {
            let mut exprs = vec![expr];
            while self.current == Token::DoubleQuestion {
                self.advance()?;
                exprs.push(self.parse_comparison()?);
            }
            expr = FormatExpr::Coalesce { exprs };
        }

        Ok(expr)
    }

    /// comparison = math (("==" | "!=" | "<" | "<=" | ">" | ">=") math)?
    fn parse_comparison(&mut self) -> Result<FormatExpr, ParseError> {
        let left = self.parse_math()?;

        let op = match &self.current {
            Token::Eq => Some(CompareOp::Eq),
            Token::Ne => Some(CompareOp::Ne),
            Token::Lt => Some(CompareOp::Lt),
            Token::Le => Some(CompareOp::Le),
            Token::Gt => Some(CompareOp::Gt),
            Token::Ge => Some(CompareOp::Ge),
            _ => None,
        };

        if let Some(op) = op {
            self.advance()?;
            let right = self.parse_math()?;
            return Ok(FormatExpr::Compare {
                left: Box::new(left),
                op,
                right: Box::new(right),
            });
        }

        Ok(left)
    }

    /// math = term (("+"|"-") term)*
    fn parse_math(&mut self) -> Result<FormatExpr, ParseError> {
        let mut left = self.parse_term()?;

        loop {
            let op = match &self.current {
                Token::Plus => MathOp::Add,
                Token::Minus => MathOp::Sub,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_term()?;
            left = FormatExpr::Math {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// term = factor (("*"|"/") factor)*
    fn parse_term(&mut self) -> Result<FormatExpr, ParseError> {
        let mut left = self.parse_factor()?;

        loop {
            let op = match &self.current {
                Token::Star => MathOp::Mul,
                Token::Slash => MathOp::Div,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_factor()?;
            left = FormatExpr::Math {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// factor = unary
    /// Note: Format specifiers (":spec") are only allowed at the top level, not inside nested expressions
    fn parse_factor(&mut self) -> Result<FormatExpr, ParseError> {
        self.parse_unary()
    }

    /// unary = "-"? primary
    fn parse_unary(&mut self) -> Result<FormatExpr, ParseError> {
        if self.current == Token::Minus {
            self.advance()?;
            let expr = self.parse_primary()?;
            return Ok(FormatExpr::Negate(Box::new(expr)));
        }
        self.parse_primary()
    }

    /// primary = "(" expression ")" | field_path | number | string | boolean
    fn parse_primary(&mut self) -> Result<FormatExpr, ParseError> {
        match &self.current {
            Token::LParen => {
                self.advance()?;
                let expr = self.parse_ternary()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Token::Int(n) => {
                let n = *n;
                self.advance()?;
                Ok(FormatExpr::Constant(Value::Int(n)))
            }
            Token::Float(n) => {
                let n = *n;
                self.advance()?;
                Ok(FormatExpr::Constant(Value::Float(n)))
            }
            Token::String(s) => {
                let s = s.clone();
                self.advance()?;
                Ok(FormatExpr::Constant(Value::String(s)))
            }
            Token::Bool(b) => {
                let b = *b;
                self.advance()?;
                Ok(FormatExpr::Constant(Value::Bool(b)))
            }
            Token::Ident(name) => {
                let name = name.clone();
                self.advance()?;

                // Check for lookup traversal (field.lookup)
                if self.current == Token::Dot {
                    self.advance()?;
                    if let Token::Ident(lookup) = &self.current {
                        let lookup = lookup.clone();
                        self.advance()?;
                        return Ok(FormatExpr::Field(FieldPath::lookup(name, lookup)));
                    } else {
                        return Err(ParseError {
                            message: "expected field name after '.'".to_string(),
                            position: self.lexer.current_pos(),
                            context: String::new(),
                        });
                    }
                }

                Ok(FormatExpr::Field(FieldPath::simple(name)))
            }
            _ => Err(ParseError {
                message: format!("unexpected token: {:?}", self.current),
                position: self.lexer.current_pos(),
                context: String::new(),
            }),
        }
    }

    /// Parse format specifier: [","]? ["." digits]? [type_char]?
    fn parse_format_spec(&mut self) -> Result<FormatSpec, ParseError> {
        let mut spec = FormatSpec::default();

        // Check for thousands separator
        if self.current == Token::Comma {
            spec.thousands_sep = true;
            self.advance()?;
        }

        // Check for precision
        if self.current == Token::Dot {
            self.advance()?;
            if let Token::Int(n) = &self.current {
                spec.precision = Some(*n as u8);
                self.advance()?;
            }
        }

        // Check for format type
        if let Token::Ident(s) = &self.current {
            spec.format_type = match s.as_str() {
                "f" => FormatType::Float,
                "d" => FormatType::Integer,
                "date" => FormatType::Date,
                "datetime" => FormatType::DateTime,
                _ => {
                    return Err(ParseError {
                        message: format!("unknown format type: {}", s),
                        position: self.lexer.current_pos(),
                        context: String::new(),
                    })
                }
            };
            self.advance()?;
        }

        // Check for percent (special case, not an ident)
        // Percent would need special handling in lexer, skip for now

        Ok(spec)
    }
}

/// Parse an expression string
fn parse_expression(input: &str, base_pos: usize) -> Result<FormatExpr, ParseError> {
    let mut parser = Parser::new(input, base_pos)?;
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_literal_only() {
        let template = parse_template("Hello, World!").unwrap();
        assert_eq!(template.parts.len(), 1);
        assert_eq!(
            template.parts[0],
            FormatPart::Literal("Hello, World!".to_string())
        );
    }

    #[test]
    fn test_parse_simple_field() {
        let template = parse_template("${name}").unwrap();
        assert_eq!(template.parts.len(), 1);
        if let FormatPart::Expr(FormatExpr::Field(path)) = &template.parts[0] {
            assert_eq!(path.base_field(), "name");
            assert!(!path.is_lookup_traversal());
        } else {
            panic!("expected field expression");
        }
    }

    #[test]
    fn test_parse_lookup_field() {
        let template = parse_template("${accountid.name}").unwrap();
        assert_eq!(template.parts.len(), 1);
        if let FormatPart::Expr(FormatExpr::Field(path)) = &template.parts[0] {
            assert_eq!(path.base_field(), "accountid");
            assert_eq!(path.lookup_field(), Some("name"));
        } else {
            panic!("expected field expression");
        }
    }

    #[test]
    fn test_parse_mixed_literal_and_expr() {
        let template = parse_template("Name: ${name}, Age: ${age}").unwrap();
        assert_eq!(template.parts.len(), 4);
        assert_eq!(
            template.parts[0],
            FormatPart::Literal("Name: ".to_string())
        );
        assert!(matches!(template.parts[1], FormatPart::Expr(_)));
        assert_eq!(
            template.parts[2],
            FormatPart::Literal(", Age: ".to_string())
        );
        assert!(matches!(template.parts[3], FormatPart::Expr(_)));
    }

    #[test]
    fn test_parse_math() {
        let template = parse_template("${a + b}").unwrap();
        if let FormatPart::Expr(FormatExpr::Math { left, op, right }) = &template.parts[0] {
            assert!(matches!(left.as_ref(), FormatExpr::Field(_)));
            assert_eq!(*op, MathOp::Add);
            assert!(matches!(right.as_ref(), FormatExpr::Field(_)));
        } else {
            panic!("expected math expression");
        }
    }

    #[test]
    fn test_parse_math_precedence() {
        // a + b * c should parse as a + (b * c)
        let template = parse_template("${a + b * c}").unwrap();
        if let FormatPart::Expr(FormatExpr::Math { left, op, right }) = &template.parts[0] {
            assert!(matches!(left.as_ref(), FormatExpr::Field(_)));
            assert_eq!(*op, MathOp::Add);
            // right should be b * c
            assert!(matches!(right.as_ref(), FormatExpr::Math { op: MathOp::Mul, .. }));
        } else {
            panic!("expected math expression");
        }
    }

    #[test]
    fn test_parse_comparison() {
        let template = parse_template("${a == 0}").unwrap();
        if let FormatPart::Expr(FormatExpr::Compare { left, op, right }) = &template.parts[0] {
            assert!(matches!(left.as_ref(), FormatExpr::Field(_)));
            assert_eq!(*op, CompareOp::Eq);
            assert!(matches!(right.as_ref(), FormatExpr::Constant(Value::Int(0))));
        } else {
            panic!("expected comparison expression");
        }
    }

    #[test]
    fn test_parse_ternary() {
        let template = parse_template("${active ? 'Yes' : 'No'}").unwrap();
        if let FormatPart::Expr(FormatExpr::Ternary {
            condition,
            then_expr,
            else_expr,
        }) = &template.parts[0]
        {
            assert!(matches!(condition.as_ref(), FormatExpr::Field(_)));
            assert!(matches!(then_expr.as_ref(), FormatExpr::Constant(Value::String(_))));
            assert!(matches!(else_expr.as_ref(), FormatExpr::Constant(Value::String(_))));
        } else {
            panic!("expected ternary expression");
        }
    }

    #[test]
    fn test_parse_coalesce() {
        let template = parse_template("${a ?? b ?? 'default'}").unwrap();
        if let FormatPart::Expr(FormatExpr::Coalesce { exprs }) = &template.parts[0] {
            assert_eq!(exprs.len(), 3);
        } else {
            panic!("expected coalesce expression");
        }
    }

    #[test]
    fn test_parse_format_spec() {
        let template = parse_template("${price:,.2f}").unwrap();
        if let FormatPart::Expr(FormatExpr::Formatted { expr, spec }) = &template.parts[0] {
            assert!(matches!(expr.as_ref(), FormatExpr::Field(_)));
            assert!(spec.thousands_sep);
            assert_eq!(spec.precision, Some(2));
            assert_eq!(spec.format_type, FormatType::Float);
        } else {
            panic!("expected formatted expression");
        }
    }

    #[test]
    fn test_parse_complex_expression() {
        // ${statecode == 0 ? 'Active' : 'Inactive'}
        let template = parse_template("${statecode == 0 ? 'Active' : 'Inactive'}").unwrap();
        if let FormatPart::Expr(FormatExpr::Ternary { condition, .. }) = &template.parts[0] {
            assert!(matches!(condition.as_ref(), FormatExpr::Compare { op: CompareOp::Eq, .. }));
        } else {
            panic!("expected ternary with comparison condition");
        }
    }

    #[test]
    fn test_parse_nested_parens() {
        let template = parse_template("${(a + b) * c}").unwrap();
        if let FormatPart::Expr(FormatExpr::Math { left, op, right }) = &template.parts[0] {
            assert_eq!(*op, MathOp::Mul);
            // left should be (a + b)
            assert!(matches!(left.as_ref(), FormatExpr::Math { op: MathOp::Add, .. }));
            assert!(matches!(right.as_ref(), FormatExpr::Field(_)));
        } else {
            panic!("expected math expression");
        }
    }

    #[test]
    fn test_parse_negation() {
        let template = parse_template("${-price}").unwrap();
        assert!(matches!(
            template.parts[0],
            FormatPart::Expr(FormatExpr::Negate(_))
        ));
    }

    #[test]
    fn test_parse_string_literal() {
        let template = parse_template("${'hello world'}").unwrap();
        if let FormatPart::Expr(FormatExpr::Constant(Value::String(s))) = &template.parts[0] {
            assert_eq!(s, "hello world");
        } else {
            panic!("expected string constant");
        }
    }

    #[test]
    fn test_parse_boolean() {
        let template = parse_template("${true}").unwrap();
        assert!(matches!(
            template.parts[0],
            FormatPart::Expr(FormatExpr::Constant(Value::Bool(true)))
        ));
    }

    #[test]
    fn test_parse_unclosed_expression_error() {
        let result = parse_template("${name");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("unclosed"));
    }

    #[test]
    fn test_parse_unclosed_string_error() {
        let result = parse_template("${'unclosed}");
        assert!(result.is_err());
    }

    #[test]
    fn test_dollar_without_brace_is_literal() {
        let template = parse_template("$100").unwrap();
        assert_eq!(template.parts.len(), 1);
        assert_eq!(template.parts[0], FormatPart::Literal("$100".to_string()));
    }
}
