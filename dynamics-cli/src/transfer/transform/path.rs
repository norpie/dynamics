//! FieldPath resolution from source records

use crate::transfer::{FieldPath, Value};

/// Resolve a field path from a source record
///
/// For simple paths like "name", extracts record["name"]
/// For lookup paths like "accountid.name", extracts record["accountid"]["name"]
/// (assumes the lookup was $expanded in the query)
pub fn resolve_path(record: &serde_json::Value, path: &FieldPath) -> Value {
    let segments = path.segments();

    if segments.is_empty() {
        return Value::Null;
    }

    // Navigate through the path
    let mut current = record;
    for segment in segments {
        match current.get(segment) {
            Some(val) => current = val,
            None => return Value::Null,
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
}
