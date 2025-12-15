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
        match current.get(segment.as_str()) {
            Some(val) => current = val,
            None => {
                // For the first segment (base field), try OData lookup value format: _fieldname_value
                // This handles cases where we're copying a lookup field's GUID value
                if i == 0 && segments.len() == 1 {
                    let value_field = format!("_{}_value", segment);
                    if let Some(val) = record.get(&value_field) {
                        return Value::from_json(val);
                    }
                }
                return Value::Null;
            }
        }
    }

    Value::from_json(current)
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
}
