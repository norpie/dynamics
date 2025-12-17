//! FieldPath resolution from source records

use crate::transfer::{FieldPath, Value};

/// Resolve a field path from a source record
///
/// For simple paths like "name", extracts record["name"]
/// For lookup paths like "accountid.name", extracts record["accountid"]["name"]
/// (assumes the lookup was $expanded in the query)
/// For lookup value paths like "accountid", also checks "_accountid_value" (OData format)
pub fn resolve_path(record: &serde_json::Value, path: &FieldPath) -> Value {
    let segments = path.segments();

    if segments.is_empty() {
        return Value::Null;
    }

    // Navigate through the path
    let mut current = record;
    for (i, segment) in segments.iter().enumerate() {
        // Try exact match first
        if let Some(val) = current.get(segment.as_str()) {
            current = val;
            continue;
        }

        // Try schema-cased navigation property name (e.g., nrq_deadlineid -> nrq_DeadlineId)
        // OData expand uses schema names which have different casing
        if let Some(val) = find_case_insensitive(current, segment) {
            current = val;
            continue;
        }

        // Try OData lookup value format: _fieldname_value
        // This handles lookup fields at any nesting level (e.g., cgk_deadlineid.cgk_projectmanagerid)
        let value_field = format!("_{}_value", segment);
        if let Some(val) = current.get(&value_field) {
            // If this is the final segment, return the value
            if i == segments.len() - 1 {
                return Value::from_json(val);
            }
            // Otherwise continue navigating (though _value fields are terminal, so this is unlikely)
            current = val;
            continue;
        }

        return Value::Null;
    }

    Value::from_json(current)
}

/// Find a key in a JSON object using case-insensitive matching
fn find_case_insensitive<'a>(obj: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let obj = obj.as_object()?;
    let key_lower = key.to_lowercase();
    for (k, v) in obj.iter() {
        if k.to_lowercase() == key_lower {
            return Some(v);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_simple_path() {
        let record = json!({
            "name": "Contoso",
            "revenue": 1000000
        });

        let path = FieldPath::simple("name");
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::String("Contoso".into()));

        let path = FieldPath::simple("revenue");
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::Int(1000000));
    }

    #[test]
    fn test_resolve_missing_field() {
        let record = json!({"name": "Contoso"});
        let path = FieldPath::simple("missing");
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::Null);
    }

    #[test]
    fn test_resolve_lookup_path() {
        let record = json!({
            "fullname": "John Doe",
            "parentcustomerid": {
                "name": "Contoso",
                "accountnumber": "ACC-001"
            }
        });

        let path = FieldPath::lookup("parentcustomerid", "name");
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::String("Contoso".into()));
    }

    #[test]
    fn test_resolve_null_lookup() {
        let record = json!({
            "fullname": "John Doe",
            "parentcustomerid": null
        });

        let path = FieldPath::lookup("parentcustomerid", "name");
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::Null);
    }

    #[test]
    fn test_resolve_three_level_path() {
        let record = json!({
            "userid": {
                "contactid": {
                    "emailaddress1": "user@example.com"
                }
            }
        });

        let path = FieldPath::parse("userid.contactid.emailaddress1").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::String("user@example.com".into()));
    }

    #[test]
    fn test_resolve_four_level_path() {
        let record = json!({
            "userid": {
                "contactid": {
                    "parentcustomerid": {
                        "name": "Acme Corp"
                    }
                }
            }
        });

        let path = FieldPath::parse("userid.contactid.parentcustomerid.name").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::String("Acme Corp".into()));
    }

    #[test]
    fn test_resolve_null_at_second_level() {
        let record = json!({
            "userid": {
                "contactid": null
            }
        });

        let path = FieldPath::parse("userid.contactid.emailaddress1").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::Null);
    }

    #[test]
    fn test_resolve_null_at_third_level() {
        let record = json!({
            "userid": {
                "contactid": {
                    "parentcustomerid": null
                }
            }
        });

        let path = FieldPath::parse("userid.contactid.parentcustomerid.name").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::Null);
    }

    #[test]
    fn test_resolve_missing_at_any_level() {
        let record = json!({
            "userid": {
                "fullname": "John"
            }
        });

        // Missing at second level
        let path = FieldPath::parse("userid.contactid.email").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::Null);

        // Missing at first level
        let path = FieldPath::parse("ownerid.name").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::Null);
    }

    #[test]
    fn test_resolve_nested_lookup_odata_value_format() {
        use uuid::Uuid;

        // Simulates OData response where expanded entity has lookup GUIDs as _fieldname_value
        let record = json!({
            "cgk_deadlineid": {
                "cgk_name": "Test Deadline",
                "_cgk_projectmanagerid_value": "ed9ceb9d-6c0f-e911-a957-000d3ab5228b",
                "_cgk_commissionid_value": "83577a7c-e621-e811-8114-5065f38b4641"
            }
        });

        // User writes path as schema field name, but data has _value format
        let path = FieldPath::parse("cgk_deadlineid.cgk_projectmanagerid").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(
            value,
            Value::Guid(Uuid::parse_str("ed9ceb9d-6c0f-e911-a957-000d3ab5228b").unwrap())
        );

        // Also test another nested lookup
        let path = FieldPath::parse("cgk_deadlineid.cgk_commissionid").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(
            value,
            Value::Guid(Uuid::parse_str("83577a7c-e621-e811-8114-5065f38b4641").unwrap())
        );

        // Regular field in expanded entity should still work
        let path = FieldPath::parse("cgk_deadlineid.cgk_name").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::String("Test Deadline".into()));
    }

    #[test]
    fn test_resolve_schema_cased_navigation_property() {
        use uuid::Uuid;

        // OData $expand returns navigation properties with schema casing (e.g., nrq_DeadlineId)
        // but users write paths with logical names (e.g., nrq_deadlineid)
        let record = json!({
            "_nrq_deadlineid_value": "f5c89a81-dab3-f011-bbd2-000d3a44517e",
            "nrq_DeadlineId": {
                "_nrq_typeid_value": "c49cb12c-7481-ef11-ac21-000d3a39e654",
                "nrq_name": "Test Deadline"
            },
            "nrq_requestid": "f9454c2d-e6b3-f011-bbd2-000d3a44517e"
        });

        // User writes path with logical name, data has schema-cased key
        let path = FieldPath::parse("nrq_deadlineid.nrq_typeid").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(
            value,
            Value::Guid(Uuid::parse_str("c49cb12c-7481-ef11-ac21-000d3a39e654").unwrap())
        );

        // Regular field in schema-cased expanded entity
        let path = FieldPath::parse("nrq_deadlineid.nrq_name").unwrap();
        let value = resolve_path(&record, &path);
        assert_eq!(value, Value::String("Test Deadline".into()));
    }
}
