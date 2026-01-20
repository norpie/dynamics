use std::collections::HashMap;

/// Type of field match/mapping
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchType {
    Exact,                        // Exact name match, types match
    Prefix,                       // Prefix name match, types match
    TypeMismatch(Box<MatchType>), // Name match but types differ - wraps underlying match type
    Manual,                       // User-created mapping (overrides type checking)
    ExampleValue,                 // Value-based match from example data
    Import,                       // Imported from C# mapping file
}

impl MatchType {
    /// Get display label for match type
    pub fn label(&self) -> String {
        match self {
            MatchType::Exact => "[Exact]".to_string(),
            MatchType::Prefix => "[Prefix]".to_string(),
            MatchType::TypeMismatch(inner) => {
                // Display as "[Prefix - Type Mismatch]" or "[Exact - Type Mismatch]"
                let inner_label_full = inner.label();
                let inner_label = inner_label_full.trim_matches(|c| c == '[' || c == ']');
                format!("[{} - Type Mismatch]", inner_label)
            }
            MatchType::Manual => "[Manual]".to_string(),
            MatchType::ExampleValue => "[Example]".to_string(),
            MatchType::Import => "[Import]".to_string(),
        }
    }
}

/// Information about a field/relationship/entity match
#[derive(Debug, Clone)]
pub struct MatchInfo {
    pub target_fields: Vec<String>, // List of target field names
    pub match_types: HashMap<String, MatchType>, // target_field -> match_type
    pub confidences: HashMap<String, f64>, // target_field -> confidence
}

impl MatchInfo {
    /// Create a new MatchInfo with a single target (common case)
    pub fn single(target_field: String, match_type: MatchType, confidence: f64) -> Self {
        let mut match_types = HashMap::new();
        match_types.insert(target_field.clone(), match_type);

        let mut confidences = HashMap::new();
        confidences.insert(target_field.clone(), confidence);

        Self {
            target_fields: vec![target_field],
            match_types,
            confidences,
        }
    }

    /// Add a target to this match info
    pub fn add_target(&mut self, target_field: String, match_type: MatchType, confidence: f64) {
        if !self.target_fields.contains(&target_field) {
            self.target_fields.push(target_field.clone());
            self.match_types.insert(target_field.clone(), match_type);
            self.confidences.insert(target_field, confidence);
        }
    }

    /// Remove a specific target
    pub fn remove_target(&mut self, target_field: &str) {
        self.target_fields.retain(|t| t != target_field);
        self.match_types.remove(target_field);
        self.confidences.remove(target_field);
    }

    /// Get the primary (first) target field
    pub fn primary_target(&self) -> Option<&String> {
        self.target_fields.first()
    }

    /// Check if this match info contains a specific target
    pub fn has_target(&self, target: &str) -> bool {
        self.target_fields.iter().any(|t| t == target)
    }

    /// Get the number of targets
    pub fn target_count(&self) -> usize {
        self.target_fields.len()
    }
}
