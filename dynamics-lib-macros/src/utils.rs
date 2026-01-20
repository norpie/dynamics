use syn::{Attribute, Expr, ExprLit, Lit};

/// Extract string value from attribute like `message = "text"`
pub fn get_attr_string(attr: &Attribute, key: &str) -> Option<String> {
    let mut result = None;
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident(key) {
            let value = meta.value()?;
            let lit: ExprLit = value.parse()?;
            if let Expr::Lit(expr_lit) = &Expr::Lit(lit)
                && let Lit::Str(lit_str) = &expr_lit.lit
            {
                result = Some(lit_str.value());
            }
        }
        Ok(())
    });
    result
}

/// Check if attribute contains a specific flag like `required` or `not_empty`
pub fn has_attr_flag(attr: &Attribute, flag: &str) -> bool {
    let mut found = false;
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident(flag) {
            found = true;
        }
        Ok(())
    });
    found
}
