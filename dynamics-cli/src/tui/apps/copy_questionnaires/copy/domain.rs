/// Domain model representing the complete questionnaire structure
/// This is what will actually be copied, and what the tree view displays
use serde_json::Value;

/// Complete questionnaire with all related entities in hierarchical structure
#[derive(Clone, Debug)]
pub struct Questionnaire {
    pub id: String,
    pub name: String,
    pub raw: Value,
    pub pages: Vec<Page>,
    pub page_lines: Vec<Value>,  // Junction records with ordering
    pub group_lines: Vec<Value>, // Junction records with ordering
    pub template_lines: Vec<TemplateLine>,
    pub conditions: Vec<Condition>,
    pub classifications: Classifications,
}

/// A page contains groups
#[derive(Clone, Debug)]
pub struct Page {
    pub id: String,
    pub name: String,
    pub order: Option<i32>,
    pub raw: Value,
    pub groups: Vec<Group>,
}

/// A group contains questions
#[derive(Clone, Debug)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub order: Option<i32>,
    pub raw: Value,
    pub questions: Vec<Question>,
}

/// A question with its tag and template references
#[derive(Clone, Debug)]
pub struct Question {
    pub id: String,
    pub name: String,
    pub raw: Value,
    pub tag: Option<Reference>,
    pub template: Option<Reference>,
}

/// A reference to a shared entity (tag, template, classification)
#[derive(Clone, Debug)]
pub struct Reference {
    pub id: String,
    pub name: Option<String>, // Will be populated if we expand
}

/// Template line linking template to group
#[derive(Clone, Debug)]
pub struct TemplateLine {
    pub id: String,
    pub raw: Value,
    pub template: Reference,
    pub group_id: String,
}

/// Condition with its actions
#[derive(Clone, Debug)]
pub struct Condition {
    pub id: String,
    pub name: String,
    pub raw: Value,
    pub actions: Vec<ConditionAction>,
}

#[derive(Clone, Debug)]
pub struct ConditionAction {
    pub id: String,
    pub name: String,
    pub raw: Value,
}

/// All N:N classification relationships
#[derive(Clone, Debug)]
pub struct Classifications {
    pub categories: Vec<Reference>,
    pub domains: Vec<Reference>,
    pub funds: Vec<Reference>,
    pub supports: Vec<Reference>,
    pub types: Vec<Reference>,
    pub subcategories: Vec<Reference>,
    pub flemish_shares: Vec<Reference>,
}

impl Questionnaire {
    pub fn total_entities(&self) -> usize {
        let mut total = 1; // questionnaire itself

        for page in &self.pages {
            total += 1; // page
            for group in &page.groups {
                total += 1; // group
                total += group.questions.len();
            }
        }

        total += self.page_lines.len();
        total += self.group_lines.len();
        total += self.template_lines.len();

        for condition in &self.conditions {
            total += 1; // condition
            total += condition.actions.len();
        }

        total += self.classifications.categories.len();
        total += self.classifications.domains.len();
        total += self.classifications.funds.len();
        total += self.classifications.supports.len();
        total += self.classifications.types.len();
        total += self.classifications.subcategories.len();
        total += self.classifications.flemish_shares.len();

        total
    }
}
