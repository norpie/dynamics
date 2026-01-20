// Test file for ResourceHandlers derive macro
use crate::tui::{Command, Resource};
use dynamics_lib_macros::ResourceHandlers;

// Mock async functions that would normally fetch data
async fn fetch_user_data() -> Result<String, String> {
    Ok("John Doe".to_string())
}

async fn fetch_items() -> Result<Vec<String>, String> {
    Ok(vec!["item1".to_string(), "item2".to_string()])
}

#[derive(Clone)]
enum Msg {
    LoadUserData,
    UserDataLoaded(Result<String, String>),
    LoadItems,
    ItemsLoaded(Result<Vec<String>, String>),
    DataReady,
}

#[derive(ResourceHandlers)]
struct TestState {
    #[resource(loader = "fetch_user_data")]
    user_data: Resource<String>,

    #[resource(loader = "fetch_items", on_complete = "DataReady")]
    items: Resource<Vec<String>>,

    // Regular field (not a resource)
    counter: usize,
}

impl Default for TestState {
    fn default() -> Self {
        Self {
            user_data: Resource::NotAsked,
            items: Resource::NotAsked,
            counter: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generated_methods_exist() {
        let mut state = TestState::default();

        // Test that load methods exist and can be called
        let _cmd1 = state.load_user_data();
        let _cmd2 = state.load_items();

        // Test that handle methods exist and can be called
        let _cmd3 = state.handle_user_data_loaded(Ok("test".to_string()));
        let _cmd4 = state.handle_items_loaded(Ok(vec!["a".to_string()]));
    }

    #[test]
    fn test_load_sets_loading_state() {
        let mut state = TestState::default();

        // Initially NotAsked
        assert!(matches!(state.user_data, Resource::NotAsked));

        // After load, should be Loading
        let _cmd = state.load_user_data();
        assert!(matches!(state.user_data, Resource::Loading));
    }

    #[test]
    fn test_handle_sets_success_state() {
        let mut state = TestState::default();

        // Handle successful result
        let _cmd = state.handle_user_data_loaded(Ok("John".to_string()));

        // Should be Success
        assert!(state.user_data.is_success());
        assert_eq!(state.user_data.as_ref().ok(), Some(&"John".to_string()));
    }

    #[test]
    fn test_handle_sets_failure_state() {
        let mut state = TestState::default();

        // Handle error result
        let _cmd = state.handle_user_data_loaded(Err("Network error".to_string()));

        // Should be Failure
        assert!(matches!(state.user_data, Resource::Failure(_)));
    }
}
