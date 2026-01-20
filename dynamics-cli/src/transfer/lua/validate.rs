//! Validation for Lua transform scripts
//!
//! Validates script syntax, structure, and declaration contents.

use anyhow::Result;

use super::runtime::LuaRuntime;
use super::types::Declaration;

/// Result of script validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the script is valid
    pub is_valid: bool,
    /// Parsed declaration (if script is valid)
    pub declaration: Option<Declaration>,
    /// Validation errors
    pub errors: Vec<ValidationError>,
    /// Validation warnings (non-fatal issues)
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn success(declaration: Declaration) -> Self {
        ValidationResult {
            is_valid: true,
            declaration: Some(declaration),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create a failed validation result with a single error
    pub fn error(message: impl Into<String>) -> Self {
        ValidationResult {
            is_valid: false,
            declaration: None,
            errors: vec![ValidationError::new(message)],
            warnings: Vec::new(),
        }
    }

    /// Add a warning
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Add an error
    pub fn with_error(mut self, error: ValidationError) -> Self {
        self.is_valid = false;
        self.errors.push(error);
        self
    }
}

/// A single validation error
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Error message
    pub message: String,
    /// Line number (if available)
    pub line: Option<usize>,
    /// Column number (if available)
    pub column: Option<usize>,
}

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        ValidationError {
            message: message.into(),
            line: None,
            column: None,
        }
    }

    pub fn with_location(mut self, line: usize, column: usize) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(line) = self.line {
            if let Some(col) = self.column {
                write!(f, "Line {}, Col {}: {}", line, col, self.message)
            } else {
                write!(f, "Line {}: {}", line, self.message)
            }
        } else {
            write!(f, "{}", self.message)
        }
    }
}

/// Validate a Lua transform script
pub fn validate_script(script: &str) -> ValidationResult {
    // Check for empty script
    if script.trim().is_empty() {
        return ValidationResult::error("Script is empty");
    }

    // Create runtime and load script
    let runtime = match LuaRuntime::new() {
        Ok(r) => r,
        Err(e) => return ValidationResult::error(format!("Failed to create Lua runtime: {}", e)),
    };

    // Try to load the script
    let module = match runtime.load_script(script) {
        Ok(m) => m,
        Err(e) => {
            let error = parse_lua_error(&e.to_string());
            return ValidationResult {
                is_valid: false,
                declaration: None,
                errors: vec![error],
                warnings: Vec::new(),
            };
        }
    };

    // Try to run declare()
    let declaration = match runtime.run_declare(&module) {
        Ok(d) => d,
        Err(e) => {
            let error = parse_lua_error(&e.to_string());
            return ValidationResult {
                is_valid: false,
                declaration: None,
                errors: vec![error],
                warnings: Vec::new(),
            };
        }
    };

    // Validate the declaration
    let mut result = ValidationResult::success(declaration.clone());

    // Check for at least one source entity
    if declaration.source.is_empty() {
        result.warnings.push(
            "No source entities declared - script will receive empty source data".to_string(),
        );
    }

    // Validate each entity declaration
    for (entity_name, entity_decl) in &declaration.source {
        if entity_decl.fields.is_empty() {
            result.warnings.push(format!(
                "Source entity '{}' has no fields declared - will receive all fields",
                entity_name
            ));
        }

        // Validate OData filter syntax (basic check)
        if let Some(filter) = &entity_decl.filter {
            if let Err(e) = validate_odata_filter(filter) {
                result = result.with_error(ValidationError::new(format!(
                    "Invalid filter for source.{}: {}",
                    entity_name, e
                )));
            }
        }
    }

    for (entity_name, entity_decl) in &declaration.target {
        if entity_decl.fields.is_empty() {
            result.warnings.push(format!(
                "Target entity '{}' has no fields declared - will receive all fields",
                entity_name
            ));
        }

        if let Some(filter) = &entity_decl.filter {
            if let Err(e) = validate_odata_filter(filter) {
                result = result.with_error(ValidationError::new(format!(
                    "Invalid filter for target.{}: {}",
                    entity_name, e
                )));
            }
        }
    }

    result
}

/// Parse Lua error message to extract line numbers
fn parse_lua_error(error: &str) -> ValidationError {
    // Lua errors often look like: "[string \"...\"]:5: error message"
    // or "runtime error: [string \"...\"]:5: error message"

    let line_regex = regex::Regex::new(r":(\d+):").ok();

    if let Some(re) = line_regex {
        if let Some(captures) = re.captures(error) {
            if let Some(line_match) = captures.get(1) {
                if let Ok(line) = line_match.as_str().parse::<usize>() {
                    return ValidationError::new(error.to_string()).with_line(line);
                }
            }
        }
    }

    ValidationError::new(error.to_string())
}

/// Basic validation of OData filter syntax
fn validate_odata_filter(filter: &str) -> Result<(), String> {
    let filter = filter.trim();

    if filter.is_empty() {
        return Ok(());
    }

    // Check for balanced parentheses
    let mut paren_count = 0;
    for ch in filter.chars() {
        match ch {
            '(' => paren_count += 1,
            ')' => {
                paren_count -= 1;
                if paren_count < 0 {
                    return Err("Unbalanced parentheses".to_string());
                }
            }
            _ => {}
        }
    }
    if paren_count != 0 {
        return Err("Unbalanced parentheses".to_string());
    }

    // Check for balanced quotes
    let mut in_string = false;
    for ch in filter.chars() {
        if ch == '\'' {
            in_string = !in_string;
        }
    }
    if in_string {
        return Err("Unterminated string literal".to_string());
    }

    // Check for common OData operators (basic sanity check)
    let valid_operators = [
        " eq ",
        " ne ",
        " gt ",
        " ge ",
        " lt ",
        " le ",
        " and ",
        " or ",
        " not ",
        " contains(",
        " startswith(",
        " endswith(",
    ];

    let has_operator = valid_operators
        .iter()
        .any(|op| filter.to_lowercase().contains(op));

    // If there's no operator, it might be a simple value or invalid
    if !has_operator && !filter.contains('(') {
        // Check if it looks like a comparison without spaces
        let comparison_chars = ['=', '<', '>'];
        if !comparison_chars.iter().any(|c| filter.contains(*c)) {
            return Err("Filter doesn't appear to contain valid OData operators".to_string());
        }
    }

    Ok(())
}

/// Validate that a script can execute without data
/// (dry run to catch runtime errors in the script structure)
pub fn validate_script_execution(script: &str) -> ValidationResult {
    let basic_result = validate_script(script);

    if !basic_result.is_valid {
        return basic_result;
    }

    // Try executing transform with empty data
    let runtime = match LuaRuntime::new() {
        Ok(r) => r,
        Err(e) => return ValidationResult::error(format!("Failed to create Lua runtime: {}", e)),
    };

    let module = match runtime.load_script(script) {
        Ok(m) => m,
        Err(e) => return ValidationResult::error(format!("Failed to load script: {}", e)),
    };

    // Run with empty data to check for obvious errors
    let empty_data = serde_json::json!({});
    match runtime.run_transform(&module, &empty_data, &empty_data) {
        Ok(_) => basic_result,
        Err(e) => {
            // Transform errors with empty data are usually warnings, not errors
            // (since the script might expect data to be present)
            basic_result.with_warning(format!(
                "Transform failed with empty data (may be expected): {}",
                e
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_script() {
        let result = validate_script("");
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_validate_minimal_script() {
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            function M.transform(source, target) return {} end
            return M
        "#;

        let result = validate_script(script);
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_script_with_syntax_error() {
        let script = r#"
            local M = {}
            function M.declare() return { source = {} target = {} } end -- missing comma
            function M.transform(source, target) return {} end
            return M
        "#;

        let result = validate_script(script);
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_validate_script_missing_declare() {
        let script = r#"
            local M = {}
            function M.transform(source, target) return {} end
            return M
        "#;

        let result = validate_script(script);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_validate_script_missing_transform() {
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            return M
        "#;

        let result = validate_script(script);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_validate_declaration_with_filter() {
        let script = r#"
            local M = {}
            function M.declare()
                return {
                    source = {
                        account = {
                            fields = { "name" },
                            filter = "statecode eq 0"
                        }
                    },
                    target = {}
                }
            end
            function M.transform(source, target) return {} end
            return M
        "#;

        let result = validate_script(script);
        assert!(result.is_valid);
    }

    #[test]
    fn test_validate_invalid_filter() {
        let script = r#"
            local M = {}
            function M.declare()
                return {
                    source = {
                        account = {
                            fields = { "name" },
                            filter = "invalid filter without operators"
                        }
                    },
                    target = {}
                }
            end
            function M.transform(source, target) return {} end
            return M
        "#;

        let result = validate_script(script);
        // Should have error about invalid filter
        assert!(!result.is_valid || !result.errors.is_empty());
    }

    #[test]
    fn test_validate_filter_unbalanced_parens() {
        let result = validate_odata_filter("((name eq 'test')");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_filter_unterminated_string() {
        let result = validate_odata_filter("name eq 'test");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_filter_valid() {
        let result = validate_odata_filter("statecode eq 0 and name ne null");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_warnings() {
        let script = r#"
            local M = {}
            function M.declare()
                return {
                    source = {
                        account = { fields = {} }  -- empty fields
                    },
                    target = {}
                }
            end
            function M.transform(source, target) return {} end
            return M
        "#;

        let result = validate_script(script);
        assert!(result.is_valid);
        assert!(!result.warnings.is_empty());
    }
}
