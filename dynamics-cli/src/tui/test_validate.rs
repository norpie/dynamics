// Test file for Validate derive macro
use crate::tui::widgets::{SelectField, TextInputField};
use dynamics_lib_macros::Validate;

#[derive(Validate)]
struct TestForm {
    #[validate(not_empty, message = "Name is required")]
    name: TextInputField,

    #[validate(required, message = "Source is required")]
    source: SelectField,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_name() {
        let mut source = SelectField::default();
        source.set_value(Some("test".to_string()));

        let form = TestForm {
            name: TextInputField {
                value: String::new(),
                state: Default::default(),
            },
            source,
        };

        let result = form.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Name is required");
    }

    #[test]
    fn test_validate_missing_source() {
        let form = TestForm {
            name: TextInputField {
                value: "test".to_string(),
                state: Default::default(),
            },
            source: SelectField::default(), // None by default
        };

        let result = form.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Source is required");
    }

    #[test]
    fn test_validate_success() {
        let mut source = SelectField::default();
        source.set_value(Some("source".to_string()));

        let form = TestForm {
            name: TextInputField {
                value: "test".to_string(),
                state: Default::default(),
            },
            source,
        };

        let result = form.validate();
        assert!(result.is_ok());
    }
}
