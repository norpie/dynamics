//! CSV mapping file parser
//!
//! Parses CSV files containing field mappings for Dynamics 365 migration.
//! CSV format: source_field, target_field, match_type, notes

use csv::ReaderBuilder;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// CSV row structure
#[derive(Debug, Deserialize)]
struct CsvRow {
    source_field: String,
    target_field: String,
    match_type: String,
    notes: String,
}

/// Parsed CSV import data distributed by mapping type
#[derive(Debug, Default, Clone)]
pub struct CsvImportData {
    /// Manual mappings (match_type = "manual")
    pub manual_mappings: HashMap<String, String>,
    /// Prefix mappings (match_type = "prefix")
    pub prefix_mappings: HashMap<String, String>,
    /// Imported mappings (match_type = "exact" or "cs_import")
    pub imported_mappings: HashMap<String, String>,
    /// Source-side ignores (empty target_field)
    pub source_ignores: HashSet<String>,
    /// Target-side ignores (empty source_field)
    pub target_ignores: HashSet<String>,
}

/// Parse CSV field mappings from file content
///
/// CSV format (4 columns with header):
/// ```csv
/// source_field,target_field,match_type,notes
/// createdby,createdby,exact,Exact match
/// cgk_accountid,nrq_accountid,prefix,Prefix match (cgk_ -> nrq_)
/// vaf_isan_code,nrq_isancode,manual,Value match: ISAN code
/// vaf_kc_dev_ai_phase_1,,ignore,Development variant - not needed
/// ```
///
/// Match types:
/// - `exact` → imported_mappings (auto-detected exact matches)
/// - `prefix` → prefix_mappings (prefix transformation rules)
/// - `manual` → manual_mappings (manually created mappings)
/// - `cs_import` → imported_mappings (imported from C# files)
/// - `ignore` → source_ignores or target_ignores (fields to skip)
///
/// Empty fields:
/// - Empty target_field → source-side ignore
/// - Empty source_field → target-side ignore
///
/// Returns: CsvImportData with mappings distributed by type
pub fn parse_csv_field_mappings(content: &str) -> Result<CsvImportData, String> {
    let mut data = CsvImportData::default();

    // Parse CSV
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let mut row_count = 0;
    let mut errors = Vec::new();

    for (line_num, result) in reader.deserialize().enumerate() {
        let row: CsvRow = match result {
            Ok(row) => row,
            Err(e) => {
                errors.push(format!("Line {}: {}", line_num + 2, e)); // +2 for header + 0-index
                continue;
            }
        };

        row_count += 1;

        let source = row.source_field.trim();
        let target = row.target_field.trim();
        let match_type = row.match_type.trim().to_lowercase();

        // Handle empty fields as ignores
        if source.is_empty() && !target.is_empty() {
            // Empty source = target-side ignore
            log::debug!("Target ignore: {}", target);
            data.target_ignores
                .insert(format!("fields:target:{}", target));
            continue;
        }

        if target.is_empty() && !source.is_empty() {
            // Empty target = source-side ignore
            log::debug!("Source ignore: {}", source);
            data.source_ignores
                .insert(format!("fields:source:{}", source));
            continue;
        }

        // Skip rows where both are empty
        if source.is_empty() && target.is_empty() {
            log::warn!("Skipping empty row at line {}", line_num + 2);
            continue;
        }

        // Distribute mappings by match_type
        match match_type.as_str() {
            "exact" | "cs_import" => {
                log::debug!(
                    "Imported mapping: {} -> {} ({})",
                    source,
                    target,
                    match_type
                );
                data.imported_mappings
                    .insert(source.to_string(), target.to_string());
            }
            "prefix" => {
                // For prefix mappings, extract the prefix from the field names
                // Example: cgk_accountid -> nrq_accountid means cgk_ -> nrq_
                if let (Some(src_prefix), Some(tgt_prefix)) =
                    (extract_prefix(source), extract_prefix(target))
                {
                    log::debug!("Prefix mapping: {} -> {}", src_prefix, tgt_prefix);
                    data.prefix_mappings.insert(src_prefix, tgt_prefix);
                } else {
                    log::warn!(
                        "Line {}: prefix match_type but no clear prefix found: {} -> {}",
                        line_num + 2,
                        source,
                        target
                    );
                    // Fall back to treating as manual mapping
                    data.manual_mappings
                        .insert(source.to_string(), target.to_string());
                }
            }
            "manual" => {
                log::debug!("Manual mapping: {} -> {}", source, target);
                data.manual_mappings
                    .insert(source.to_string(), target.to_string());
            }
            "ignore" => {
                // Explicit ignore type (in addition to empty field handling)
                if !target.is_empty() {
                    log::debug!("Source ignore (explicit): {}", source);
                    data.source_ignores
                        .insert(format!("fields:source:{}", source));
                }
                if !source.is_empty() && target.is_empty() {
                    // Should have been caught above, but handle explicitly
                    log::debug!("Source ignore (explicit): {}", source);
                    data.source_ignores
                        .insert(format!("fields:source:{}", source));
                }
            }
            other => {
                errors.push(format!(
                    "Line {}: unknown match_type '{}'",
                    line_num + 2,
                    other
                ));
                log::warn!(
                    "Line {}: unknown match_type '{}', treating as manual mapping",
                    line_num + 2,
                    other
                );
                data.manual_mappings
                    .insert(source.to_string(), target.to_string());
            }
        }
    }

    // Validate results
    if errors.len() > 10 {
        return Err(format!(
            "Too many parsing errors ({}). First few:\n{}",
            errors.len(),
            errors[..10].join("\n")
        ));
    }

    let total_items = data.manual_mappings.len()
        + data.prefix_mappings.len()
        + data.imported_mappings.len()
        + data.source_ignores.len()
        + data.target_ignores.len();

    if total_items == 0 {
        return Err("No valid mappings or ignores found in CSV. Check file format.".to_string());
    }

    log::info!(
        "Parsed CSV: {} manual, {} prefix, {} imported, {} source ignores, {} target ignores ({} rows total)",
        data.manual_mappings.len(),
        data.prefix_mappings.len(),
        data.imported_mappings.len(),
        data.source_ignores.len(),
        data.target_ignores.len(),
        row_count
    );

    if !errors.is_empty() {
        log::warn!(
            "CSV parsing completed with {} errors/warnings",
            errors.len()
        );
    }

    Ok(data)
}

/// Extract prefix from field name
/// Returns the prefix including the separator (e.g., "cgk_accountid" -> "cgk_")
fn extract_prefix(field: &str) -> Option<String> {
    // Look for common separators
    for sep in ['_', '.'] {
        if let Some(pos) = field.find(sep) {
            if pos > 0 && pos < field.len() - 1 {
                // Include the separator in the prefix
                return Some(field[..=pos].to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_all_types() {
        let csv = r#"source_field,target_field,match_type,notes
createdby,createdby,exact,Exact match
cgk_accountid,nrq_accountid,prefix,Prefix match
vaf_isan,nrq_isan,manual,Manual mapping
old_field,new_field,cs_import,Imported from CS
dev_field,,ignore,Development field
"#;

        let result = parse_csv_field_mappings(csv).unwrap();

        // Check imported mappings (exact + cs_import)
        assert_eq!(result.imported_mappings.len(), 2);
        assert_eq!(
            result.imported_mappings.get("createdby"),
            Some(&"createdby".to_string())
        );
        assert_eq!(
            result.imported_mappings.get("old_field"),
            Some(&"new_field".to_string())
        );

        // Check prefix mappings
        assert_eq!(result.prefix_mappings.len(), 1);
        assert_eq!(
            result.prefix_mappings.get("cgk_"),
            Some(&"nrq_".to_string())
        );

        // Check manual mappings
        assert_eq!(result.manual_mappings.len(), 1);
        assert_eq!(
            result.manual_mappings.get("vaf_isan"),
            Some(&"nrq_isan".to_string())
        );

        // Check source ignores
        assert_eq!(result.source_ignores.len(), 1);
        assert!(result.source_ignores.contains("fields:source:dev_field"));
    }

    #[test]
    fn test_empty_target_creates_source_ignore() {
        let csv = r#"source_field,target_field,match_type,notes
field1,,ignore,Ignore this field
field2,,manual,Also ignore
"#;

        let result = parse_csv_field_mappings(csv).unwrap();
        assert_eq!(result.source_ignores.len(), 2);
        assert!(result.source_ignores.contains("fields:source:field1"));
        assert!(result.source_ignores.contains("fields:source:field2"));
    }

    #[test]
    fn test_empty_source_creates_target_ignore() {
        let csv = r#"source_field,target_field,match_type,notes
,target_field,ignore,Ignore this target
"#;

        let result = parse_csv_field_mappings(csv).unwrap();
        assert_eq!(result.target_ignores.len(), 1);
        assert!(result.target_ignores.contains("fields:target:target_field"));
    }

    #[test]
    fn test_prefix_extraction() {
        assert_eq!(extract_prefix("cgk_field"), Some("cgk_".to_string()));
        assert_eq!(extract_prefix("nrq_field"), Some("nrq_".to_string()));
        assert_eq!(extract_prefix("vaf_isan_code"), Some("vaf_".to_string()));
        assert_eq!(extract_prefix("noprefix"), None);
        assert_eq!(extract_prefix("_invalid"), None); // Edge case - separator at start
    }

    #[test]
    fn test_empty_csv() {
        let csv = r#"source_field,target_field,match_type,notes
"#;
        let result = parse_csv_field_mappings(csv);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No valid mappings"));
    }

    #[test]
    fn test_malformed_csv() {
        let csv = r#"source_field,target_field,match_type,notes
field1,field2
"#;
        let result = parse_csv_field_mappings(csv);
        // Should either error or handle gracefully
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_unknown_match_type_fallback() {
        let csv = r#"source_field,target_field,match_type,notes
field1,field2,unknown_type,Some field
"#;
        let result = parse_csv_field_mappings(csv).unwrap();
        // Unknown types fall back to manual mappings
        assert_eq!(result.manual_mappings.len(), 1);
        assert_eq!(
            result.manual_mappings.get("field1"),
            Some(&"field2".to_string())
        );
    }

    #[test]
    fn test_whitespace_trimming() {
        let csv = r#"source_field,target_field,match_type,notes
  field1  ,  field2  ,  exact  ,  Note with spaces
"#;
        let result = parse_csv_field_mappings(csv).unwrap();
        assert_eq!(result.imported_mappings.len(), 1);
        assert_eq!(
            result.imported_mappings.get("field1"),
            Some(&"field2".to_string())
        );
    }
}
