//! C# mapping file parser
//!
//! Extracts field mappings from C# Dynamics 365 migration mapping files.
//! These files contain InternalMapping methods that map source fields to target fields.

use regex::Regex;
use std::collections::HashMap;

/// Parse C# field mappings from file content
///
/// Extracts source->target field mappings from InternalMapping method body.
/// Skips commented lines and handles various C# syntax patterns including:
/// - Simple assignments: `target = source.field`
/// - Null coalescing: `target = source.field1 ?? source.field2`
/// - Ternary operators: `target = condition ? source.field1 : source.field2`
/// - Extension methods: `target = source.field?.SafeSubstring(0, 100)`
/// - Method calls: `target = Method(source.field)`
/// - Variable pre-declarations with resolution
///
/// # Example
/// ```csharp
/// protected override TargetEntity InternalMapping(SourceEntity source, MigrationOptions options)
/// {
///     var entity = new TargetEntity(source.Id)
///     {
///         nrq_Name = source.cgk_name,              // Simple mapping
///         nrq_Date = source.cgk_date,              // Another one
///         //nrq_Skipped = source.cgk_skipped,      // Commented out (ignored)
///         nrq_Fund = FundXRef.GetTargetReference(source.cgk_fundid?.Id), // Complex (extracts cgk_fundid)
///         nrq_Amount = source.vaf_amount ?? source.cgk_amount,  // Null coalescing (extracts both)
///     };
///     return entity;
/// }
/// ```
///
/// Returns: HashMap<source_field, target_field>
pub fn parse_cs_field_mappings(content: &str) -> Result<HashMap<String, String>, String> {
    let mut mappings = HashMap::new();

    // Find InternalMapping method signature
    let method_start = content
        .find("InternalMapping")
        .ok_or("InternalMapping method not found in file")?;

    // Extract source parameter name from method signature
    // Pattern: InternalMapping(Type sourceName, ...)
    let sig_pattern = Regex::new(r"InternalMapping\s*\(\s*\w+\s+(\w+)\s*,").unwrap();
    let source_var_name = if let Some(caps) = sig_pattern.captures(&content[method_start..]) {
        caps.get(1).unwrap().as_str()
    } else {
        return Err(
            "Could not extract source parameter name from InternalMapping signature".to_string(),
        );
    };

    log::debug!("Detected source variable name: {}", source_var_name);

    // Find object initializer - could be from "return new" or "var x = new"
    let method_body_start = content[method_start..]
        .find('{')
        .ok_or("Method body not found")?;
    let method_body = &content[method_start + method_body_start..];

    // PHASE 2: Parse variable declarations before initializer for later resolution
    let variable_assignments = parse_variable_assignments(method_body, source_var_name);
    log::debug!("Found {} variable assignments", variable_assignments.len());

    // Look for "new TargetType(" followed by object initializer
    let new_pattern = Regex::new(r"new\s+\w+\s*\([^)]*\)\s*\{").unwrap();
    let init_start = if let Some(mat) = new_pattern.find(method_body) {
        mat.start()
    } else {
        return Err("Object initializer not found in method body".to_string());
    };

    // Find the opening brace of the initializer
    let init_brace = method_body[init_start..]
        .find('{')
        .ok_or("Object initializer brace not found")?;
    let init_body = &method_body[init_start + init_brace..];

    // Split into lines to check for comments
    let lines: Vec<&str> = init_body.lines().collect();

    for line in lines.iter() {
        // Skip commented lines
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }

        // Extract target field (left side of assignment)
        let target_field = match extract_target_field(line) {
            Some(field) => field,
            None => continue,
        };

        // Skip system fields
        if target_field == "OverriddenCreatedOn"
            || target_field == "statecode"
            || target_field == "statuscode"
        {
            continue;
        }

        // Extract all source fields from the right side of assignment
        let source_fields = extract_source_fields(line, source_var_name, &variable_assignments);

        // Create mappings for each source field found
        for source_field in source_fields {
            // Skip system fields
            if source_field == "Id" || source_field == "CreatedOn" || source_field == "ModifiedOn" {
                continue;
            }

            log::debug!("Parsed mapping: {} -> {}", source_field, target_field);

            // For now, map each source field to target field
            // If multiple sources (like null coalescing), later ones may overwrite
            // But that's acceptable since they're alternatives
            mappings.insert(source_field.clone(), target_field.clone());
        }
    }

    if mappings.is_empty() {
        return Err("No field mappings found in file. Check file format.".to_string());
    }

    log::info!("Parsed {} field mappings from C# file", mappings.len());
    Ok(mappings)
}

/// Extract target field name from assignment line
/// Matches: targetField = ...
fn extract_target_field(line: &str) -> Option<String> {
    let target_pattern = Regex::new(r"^\s*(\w+)\s*=").unwrap();
    target_pattern
        .captures(line)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Extract all source field references from a line
/// Handles multiple patterns:
/// - Simple: source.field
/// - Null coalescing: source.field1 ?? source.field2
/// - Ternary: condition ? source.field1 : source.field2
/// - Extension methods: source.field?.SafeSubstring(0, 100)
/// - Method calls: Method(source.field)
/// - Variable references: uses variable_assignments to resolve
fn extract_source_fields(
    line: &str,
    source_var_name: &str,
    variable_assignments: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let mut fields = Vec::new();

    // Pattern to match source.field with various suffixes
    // Matches: sourceVar.fieldName followed by optional ?, ?., <T>, etc.
    let source_pattern =
        Regex::new(&format!(r"{}\.(\w+)", regex::escape(source_var_name))).unwrap();

    // Extract all matches of source.field in the line
    for capture in source_pattern.captures_iter(line) {
        if let Some(field_match) = capture.get(1) {
            let field = field_match.as_str().to_string();
            if !fields.contains(&field) {
                fields.push(field);
            }
        }
    }

    // PHASE 2: Check for variable references and resolve them
    // Pattern: matches standalone variable names that might be from pre-declarations
    let var_pattern = Regex::new(r"\b([a-z][a-zA-Z0-9]*)\b").unwrap();
    for capture in var_pattern.captures_iter(line) {
        if let Some(var_match) = capture.get(1) {
            let var_name = var_match.as_str();
            // Check if this variable was assigned from source fields
            if let Some(source_fields) = variable_assignments.get(var_name) {
                for field in source_fields {
                    if !fields.contains(field) {
                        log::debug!("Resolved variable {} to source field {}", var_name, field);
                        fields.push(field.clone());
                    }
                }
            }
        }
    }

    fields
}

/// Parse variable assignments in method body before the initializer
/// Returns a map of variable_name -> [source_fields]
///
/// Handles patterns like:
/// - var temp = source.field;
/// - EntityReference ref = null; ... ref = source.field;
/// - var x = source.field1 ?? source.field2;
/// - dictionary.TryGetValue(source.field, out var result)
fn parse_variable_assignments(
    method_body: &str,
    source_var_name: &str,
) -> HashMap<String, Vec<String>> {
    let mut assignments = HashMap::new();

    // Pattern 1: var varName = ...source.field...
    let var_decl_pattern = Regex::new(r"(?m)^\s*(?:var|[\w<>?]+)\s+(\w+)\s*=\s*(.+?);").unwrap();

    // Pattern 2: varName = ...source.field... (reassignment)
    let var_assign_pattern = Regex::new(r"(?m)^\s*(\w+)\s*=\s*(.+?);").unwrap();

    // Helper to extract source fields from expression
    let extract_fields = |expr: &str| -> Vec<String> {
        let source_pattern =
            Regex::new(&format!(r"{}\.(\w+)", regex::escape(source_var_name))).unwrap();
        source_pattern
            .captures_iter(expr)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect()
    };

    // Parse variable declarations
    for capture in var_decl_pattern.captures_iter(method_body) {
        if let (Some(var_name), Some(expression)) = (capture.get(1), capture.get(2)) {
            let var_name = var_name.as_str().to_string();
            let fields = extract_fields(expression.as_str());
            if !fields.is_empty() {
                log::debug!(
                    "Variable {} assigned from source fields: {:?}",
                    var_name,
                    fields
                );
                assignments.insert(var_name, fields);
            }
        }
    }

    // Parse variable reassignments
    for capture in var_assign_pattern.captures_iter(method_body) {
        if let (Some(var_name), Some(expression)) = (capture.get(1), capture.get(2)) {
            let var_name = var_name.as_str().to_string();
            let fields = extract_fields(expression.as_str());
            if !fields.is_empty() && !assignments.contains_key(&var_name) {
                log::debug!(
                    "Variable {} reassigned from source fields: {:?}",
                    var_name,
                    fields
                );
                assignments.insert(var_name, fields);
            }
        }
    }

    // Parse TryGetValue out parameters
    // Extract source fields from the TryGetValue call and associate with the out variable
    let trygetvalue_full_pattern =
        Regex::new(r"TryGetValue\s*\(([^,]+),\s*out\s+(?:var\s+)?(\w+)\)").unwrap();
    for capture in trygetvalue_full_pattern.captures_iter(method_body) {
        if let (Some(first_arg), Some(var_name)) = (capture.get(1), capture.get(2)) {
            let var_name = var_name.as_str().to_string();
            let fields = extract_fields(first_arg.as_str());
            if !fields.is_empty() {
                log::debug!(
                    "Variable {} from TryGetValue contains source fields: {:?}",
                    var_name,
                    fields
                );
                assignments.insert(var_name, fields);
            } else if !assignments.contains_key(&var_name) {
                // Mark as exists even if no source fields detected
                assignments.insert(var_name, vec![]);
            }
        }
    }

    assignments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_mappings() {
        let content = r#"
        protected override nrq_Deadline InternalMapping(cgk_deadline sourceDeadline, MigrationOptions options)
        {
            return new nrq_Deadline(sourceDeadline.Id)
            {
                nrq_Name = sourceDeadline.cgk_name,
                nrq_Date = sourceDeadline.cgk_date,
                nrq_CommissionId = sourceDeadline.cgk_commissionid,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();
        assert_eq!(result.get("cgk_name"), Some(&"nrq_Name".to_string()));
        assert_eq!(result.get("cgk_date"), Some(&"nrq_Date".to_string()));
        assert_eq!(
            result.get("cgk_commissionid"),
            Some(&"nrq_CommissionId".to_string())
        );
    }

    #[test]
    fn test_skip_commented_lines() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_Active = source.cgk_active,
                //nrq_Skipped = source.cgk_skipped,
                nrq_Other = source.cgk_other,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("cgk_active"));
        assert!(result.contains_key("cgk_other"));
        assert!(!result.contains_key("cgk_skipped"));
    }

    #[test]
    fn test_complex_expressions() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_Fund = FundXRef.GetTargetReference(source.cgk_fundid?.Id),
                nrq_Status = (int?)source.cgk_status,
                nrq_President = source.cgk_presidentid,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();
        assert_eq!(result.get("cgk_fundid"), Some(&"nrq_Fund".to_string()));
        assert_eq!(result.get("cgk_status"), Some(&"nrq_Status".to_string()));
        assert_eq!(
            result.get("cgk_presidentid"),
            Some(&"nrq_President".to_string())
        );
    }

    #[test]
    fn test_no_internal_mapping_method() {
        let content = "public class SomeClass { }";
        let result = parse_cs_field_mappings(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("InternalMapping"));
    }

    #[test]
    fn test_variable_assignment_pattern() {
        let content = r#"
        protected override nrq_Request InternalMapping(cgk_request sourceRequest, MigrationOptions options)
        {
            var request = new nrq_Request(sourceRequest.Id)
            {
                nrq_Name = sourceRequest.cgk_name,
                nrq_Date = sourceRequest.cgk_date,
            };
            return request;
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();
        assert_eq!(result.get("cgk_name"), Some(&"nrq_Name".to_string()));
        assert_eq!(result.get("cgk_date"), Some(&"nrq_Date".to_string()));
    }

    #[test]
    fn test_xref_patterns() {
        let content = r#"
        protected override Target InternalMapping(Source sourceEntity, MigrationOptions options)
        {
            return new Target(sourceEntity.Id)
            {
                nrq_Fund = FundXRef.GetTargetReference(sourceEntity.cgk_fundid?.Id),
                nrq_Category = CategoryXRef.GetTargetReference(sourceEntity.cgk_categoryid?.Id),
                nrq_Simple = sourceEntity.cgk_simple,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();
        assert_eq!(result.get("cgk_fundid"), Some(&"nrq_Fund".to_string()));
        assert_eq!(
            result.get("cgk_categoryid"),
            Some(&"nrq_Category".to_string())
        );
        assert_eq!(result.get("cgk_simple"), Some(&"nrq_Simple".to_string()));
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_cast_and_method_call_patterns() {
        let content = r#"
        protected override Target InternalMapping(Source src, MigrationOptions options)
        {
            var entity = new Target(src.Id)
            {
                nrq_Status = (int?)src.cgk_status,
                nrq_Project = src.cgk_projectid?.Cast<nrq_Project>(),
                nrq_Review = ConvertReview(src.cgk_review),
                nrq_Type = ConvertSubmissionType(src.vaf_type),
            };
            return entity;
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();
        assert_eq!(result.get("cgk_status"), Some(&"nrq_Status".to_string()));
        assert_eq!(
            result.get("cgk_projectid"),
            Some(&"nrq_Project".to_string())
        );
        assert_eq!(result.get("cgk_review"), Some(&"nrq_Review".to_string()));
        assert_eq!(result.get("vaf_type"), Some(&"nrq_Type".to_string()));
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_mixed_patterns_with_comments() {
        let content = r#"
        protected override nrq_Entity InternalMapping(cgk_entity sourceEntity, MigrationOptions options)
        {
            var result = new nrq_Entity(sourceEntity.Id)
            {
                nrq_Active = sourceEntity.cgk_active,
                //nrq_Disabled = sourceEntity.cgk_disabled,
                nrq_FundId = FundXRef.GetTargetReference(sourceEntity.cgk_fundid?.Id),
                //nrq_Skipped = CategoryXRef.Get(sourceEntity.cgk_skipped),
                nrq_Amount = (decimal?)sourceEntity.cgk_amount,
                nrq_Name = sourceEntity.cgk_name,
                OverriddenCreatedOn = sourceEntity.CreatedOn,
            };
            return result;
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();
        assert_eq!(result.len(), 4); // Should skip commented lines and OverriddenCreatedOn
        assert_eq!(result.get("cgk_active"), Some(&"nrq_Active".to_string()));
        assert_eq!(result.get("cgk_fundid"), Some(&"nrq_FundId".to_string()));
        assert_eq!(result.get("cgk_amount"), Some(&"nrq_Amount".to_string()));
        assert_eq!(result.get("cgk_name"), Some(&"nrq_Name".to_string()));
        assert!(!result.contains_key("cgk_disabled"));
        assert!(!result.contains_key("cgk_skipped"));
        assert!(!result.contains_key("CreatedOn"));
    }

    #[test]
    fn test_real_requests_file() {
        // Test with a subset of the real Requests.cs pattern
        let content = r#"
        public class Requests : DataverseToDataverseMapping<cgk_request, nrq_Request>
        {
            protected override nrq_Request InternalMapping(cgk_request sourceRequest, MigrationOptions options)
            {
                var request = new nrq_Request(sourceRequest.Id)
                {
                    nrq_AccountId = sourceRequest.cgk_accountid,
                    //nrq_ActiveTransferRequest
                    nrq_Adaption = sourceRequest.vaf_adaptatie,
                    nrq_Amountofepisodes = sourceRequest.cgk_amountofepisodes,
                    nrq_CategoryId = CategoryXRef.GetTargetReference(sourceRequest.cgk_categoryid?.Id),
                    nrq_Commission = sourceRequest.cgk_commissiondecision,
                    nrq_ProjectId = sourceRequest.cgk_folderid?.Cast<nrq_Project>(),
                    nrq_Review = ConvertReview(sourceRequest.cgk_review),
                    nrq_SubmissionType = ConvertSubmissionType(sourceRequest.vaf_typeindiening),
                    OverriddenCreatedOn = sourceRequest.CreatedOn,
                };
                return request;
            }
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Verify key mappings are extracted correctly
        assert!(
            result.len() >= 8,
            "Should have at least 8 mappings, got {}",
            result.len()
        );
        assert_eq!(
            result.get("cgk_accountid"),
            Some(&"nrq_AccountId".to_string())
        );
        assert_eq!(
            result.get("vaf_adaptatie"),
            Some(&"nrq_Adaption".to_string())
        );
        assert_eq!(
            result.get("cgk_amountofepisodes"),
            Some(&"nrq_Amountofepisodes".to_string())
        );
        assert_eq!(
            result.get("cgk_categoryid"),
            Some(&"nrq_CategoryId".to_string())
        );
        assert_eq!(
            result.get("cgk_commissiondecision"),
            Some(&"nrq_Commission".to_string())
        );
        assert_eq!(
            result.get("cgk_folderid"),
            Some(&"nrq_ProjectId".to_string())
        );
        assert_eq!(result.get("cgk_review"), Some(&"nrq_Review".to_string()));
        assert_eq!(
            result.get("vaf_typeindiening"),
            Some(&"nrq_SubmissionType".to_string())
        );

        // Should not include commented or system fields
        assert!(!result.contains_key("CreatedOn"));
    }

    // NEW TESTS FOR ENHANCED PATTERNS

    #[test]
    fn test_null_coalescing_operator() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_Amount = source.vaf_vrijgavebedrag ?? source.cgk_vrijgavebedrag,
                nrq_Name = source.cgk_name,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Both sides of null coalescing should be extracted
        assert!(result.contains_key("vaf_vrijgavebedrag"));
        assert!(result.contains_key("cgk_vrijgavebedrag"));
        assert!(result.contains_key("cgk_name"));

        // Both should map to nrq_Amount (one will overwrite the other, but both are valid alternatives)
        assert!(
            result.get("vaf_vrijgavebedrag") == Some(&"nrq_Amount".to_string())
                || result.get("cgk_vrijgavebedrag") == Some(&"nrq_Amount".to_string())
        );
    }

    #[test]
    fn test_ternary_operator() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                EMailAddress1 = options.AnonymizeEmailAddresses ? DataHelper.AnonymizeEmailAddress(source.EMailAddress1) : source.EMailAddress1,
                nrq_Other = source.cgk_other,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract source field from ternary operator (appears in both branches)
        assert_eq!(
            result.get("EMailAddress1"),
            Some(&"EMailAddress1".to_string())
        );
        assert_eq!(result.get("cgk_other"), Some(&"nrq_Other".to_string()));
    }

    #[test]
    fn test_ternary_with_different_fields() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_Type = source.vaf_availablein?.Value == 100000000 ? source.cgk_type1 : source.cgk_type2,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract all source fields from the ternary expression
        assert!(result.contains_key("vaf_availablein"));
        assert!(result.contains_key("cgk_type1") || result.contains_key("cgk_type2"));
    }

    #[test]
    fn test_safesubstring_extension() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                JobTitle = source.JobTitle?.SafeSubstring(0, 100),
                Description = source.Description?.SafeSubstring(0, 2000),
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract field names ignoring SafeSubstring suffix
        assert_eq!(result.get("JobTitle"), Some(&"JobTitle".to_string()));
        assert_eq!(result.get("Description"), Some(&"Description".to_string()));
    }

    #[test]
    fn test_tostring_extension() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_Seasonseries = source.vaf_Seasonseries?.ToString(),
                nrq_Other = source.cgk_other,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract field names ignoring ToString suffix
        assert_eq!(
            result.get("vaf_Seasonseries"),
            Some(&"nrq_Seasonseries".to_string())
        );
        assert_eq!(result.get("cgk_other"), Some(&"nrq_Other".to_string()));
    }

    #[test]
    fn test_variable_pre_declaration() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            var availablein = source.vaf_availablein;

            return new Target(source.Id)
            {
                nrq_Available = availablein,
                nrq_Name = source.cgk_name,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should resolve variable to source field
        assert_eq!(
            result.get("vaf_availablein"),
            Some(&"nrq_Available".to_string())
        );
        assert_eq!(result.get("cgk_name"), Some(&"nrq_Name".to_string()));
    }

    #[test]
    fn test_variable_with_null_coalescing() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            var amount = source.vaf_amount ?? source.cgk_amount;

            return new Target(source.Id)
            {
                nrq_Amount = amount,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract both source fields from the null coalescing in variable declaration
        assert!(result.contains_key("vaf_amount") || result.contains_key("cgk_amount"));
    }

    #[test]
    fn test_complex_nested_ternary() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_Name = source.new_originaltitle == null ? source.cgk_name : source.vaf_name,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract all source fields involved
        assert!(result.contains_key("new_originaltitle"));
        assert!(result.contains_key("cgk_name") || result.contains_key("vaf_name"));
    }

    #[test]
    fn test_getattributevalue_with_generic() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            var availablein = source.GetAttributeValue<OptionSetValue>(cgk_film.GetColumnName<cgk_film>(f => f.vaf_availablein));

            return new Target(source.Id)
            {
                nrq_Available = ConvertValue(source.vaf_availablein),
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract vaf_availablein field
        assert!(result.contains_key("vaf_availablein"));
    }

    #[test]
    fn test_multiple_source_fields_one_line() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_FullName = source.cgk_firstname + " " + source.cgk_lastname,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract both fields
        assert!(result.contains_key("cgk_firstname"));
        assert!(result.contains_key("cgk_lastname"));
    }

    #[test]
    fn test_hasvalue_and_value_access() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_Status = source.cgk_status.HasValue ? (int)source.cgk_status.Value : 0,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract cgk_status (appears multiple times)
        assert!(result.contains_key("cgk_status"));
        assert_eq!(result.get("cgk_status"), Some(&"nrq_Status".to_string()));
    }

    #[test]
    fn test_conditional_xref() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_Request = RequestXRef.HasReference(source.vaf_request?.Id) ? RequestXRef.GetTargetReference(source.vaf_request?.Id) : source.vaf_request?.Cast<nrq_Request>(),
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should extract vaf_request field (appears multiple times in the ternary)
        assert!(result.contains_key("vaf_request"));
        assert_eq!(result.get("vaf_request"), Some(&"nrq_Request".to_string()));
    }

    #[test]
    fn test_skip_system_fields() {
        let content = r#"
        protected override Target InternalMapping(Source source, MigrationOptions options)
        {
            return new Target(source.Id)
            {
                nrq_Name = source.cgk_name,
                OverriddenCreatedOn = source.CreatedOn,
                statecode = source.statecode,
                statuscode = source.statuscode,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should only have user fields, not system fields
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("cgk_name"), Some(&"nrq_Name".to_string()));
        assert!(!result.contains_key("CreatedOn"));
        assert!(!result.contains_key("ModifiedOn"));
        assert!(!result.contains_key("Id"));
    }

    #[test]
    fn test_trygetvalue_with_source_field() {
        let content = r#"
        protected override Target InternalMapping(Source sourceEntity, MigrationOptions options)
        {
            EntityReference contactRef = null;
            if (sourceEntity.cgk_userid != null)
            {
                _userLookup.TryGetValue(sourceEntity.cgk_userid.Id, out contactRef);
            }

            return new Target(sourceEntity.Id)
            {
                nrq_ContactId = contactRef,
                nrq_Name = sourceEntity.cgk_name,
            };
        }
        "#;

        let result = parse_cs_field_mappings(content).unwrap();

        // Should resolve contactRef to cgk_userid and extract cgk_name
        assert!(result.contains_key("cgk_userid"));
        assert!(result.contains_key("cgk_name"));
        assert_eq!(result.get("cgk_userid"), Some(&"nrq_ContactId".to_string()));
        assert_eq!(result.get("cgk_name"), Some(&"nrq_Name".to_string()));
    }
}
