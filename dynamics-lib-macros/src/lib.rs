use proc_macro::TokenStream;

mod app_state;
mod resource_handlers;
mod utils;
mod validate;

/// Derive macro for validation framework
///
/// # Example
/// ```rust,ignore
/// #[derive(Validate)]
/// struct CreateForm {
///     #[validate(not_empty, message = "Name required")]
///     name: TextInputField,
///
///     #[validate(required, message = "Source required")]
///     source: SelectField,
/// }
/// ```
#[proc_macro_derive(Validate, attributes(validate))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
    validate::derive(input)
}

/// Derive macro for Resource field handlers
///
/// Generates helper methods for loading and handling Resource fields.
///
/// # Example
/// ```rust,ignore
/// #[derive(ResourceHandlers)]
/// struct State {
///     #[resource(loader = "fetch_data")]
///     data: Resource<Vec<String>>,
/// }
///
/// // Generates:
/// // - fn load_data(&mut self) -> Command<Msg>
/// // - fn handle_data_loaded(&mut self, result: Result<Vec<String>, String>) -> Command<Msg>
/// ```
#[proc_macro_derive(ResourceHandlers, attributes(resource))]
pub fn derive_resource_handlers(input: TokenStream) -> TokenStream {
    resource_handlers::derive(input)
}

/// Derive macro for AppState trait with widget auto-routing
///
/// Automatically implements AppState::dispatch_widget_event to route events to Field types.
///
/// # Example
/// ```rust,ignore
/// #[derive(AppState)]
/// struct State {
///     #[widget("name-input")]
///     name: TextInputField,
///
///     #[widget("entity-autocomplete", options = "self.all_entities")]
///     entity: AutocompleteField,
///
///     all_entities: Vec<String>,
/// }
/// ```
#[proc_macro_derive(AppState, attributes(widget))]
pub fn derive_app_state(input: TokenStream) -> TokenStream {
    app_state::derive(input)
}
