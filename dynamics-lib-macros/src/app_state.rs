use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Data, DeriveInput, Fields, Token, Type, parse_macro_input};

/// Parse #[widget(...)] attribute
/// Supports: #[widget("id")] or #[widget("id", options = "expr")]
struct WidgetAttr {
    id: syn::LitStr,
    options: Option<syn::LitStr>,
}

impl Parse for WidgetAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let id: syn::LitStr = input.parse()?;

        let options = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;

            // Parse "options"
            let ident: syn::Ident = input.parse()?;
            if ident != "options" {
                return Err(syn::Error::new_spanned(ident, "Expected 'options'"));
            }

            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        } else {
            None
        };

        Ok(WidgetAttr { id, options })
    }
}

fn parse_widget_attr(attr: &Attribute) -> syn::Result<(String, Option<String>)> {
    let widget_attr: WidgetAttr = attr.parse_args()?;
    Ok((
        widget_attr.id.value(),
        widget_attr.options.map(|lit| lit.value()),
    ))
}

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let Data::Struct(data_struct) = &input.data else {
        return syn::Error::new_spanned(name, "AppState can only be derived for structs")
            .to_compile_error()
            .into();
    };

    let Fields::Named(fields) = &data_struct.fields else {
        return syn::Error::new_spanned(name, "AppState requires named fields")
            .to_compile_error()
            .into();
    };

    let mut match_arms = Vec::new();

    for field in fields.named.iter() {
        let field_name = field.ident.as_ref().unwrap();

        // Find #[widget(...)] attribute
        for attr in field.attrs.iter() {
            if !attr.path().is_ident("widget") {
                continue;
            }

            // Extract widget id and optional options expression
            let (widget_id, options_expr) = match parse_widget_attr(attr) {
                Ok(result) => result,
                Err(e) => return e.to_compile_error().into(),
            };

            // Determine event type and handle_event call based on field type
            let field_type = &field.ty;
            let (event_type, handle_call) =
                infer_event_type_and_handler(field_type, field_name, &options_expr);

            match_arms.push(quote! {
                #widget_id => {
                    if let Some(event) = event.downcast_ref::<#event_type>() {
                        #handle_call
                        return true;
                    }
                }
            });
        }
    }

    let expanded = quote! {
        impl crate::tui::AppState for #name {
            fn dispatch_widget_event(&mut self, id: &crate::tui::element::FocusId, event: &dyn std::any::Any) -> bool {
                match id.0 {
                    #(#match_arms)*
                    _ => {}
                }
                false
            }
        }
    };

    TokenStream::from(expanded)
}

/// Infer event type and handle_event call from field type
fn infer_event_type_and_handler(
    field_type: &Type,
    field_name: &syn::Ident,
    options_expr: &Option<String>,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let type_name = extract_type_name(field_type);

    match type_name.as_str() {
        "TextInputField" => {
            let handle = quote! {
                self.#field_name.handle_event(event.clone(), None);
            };
            (quote! { crate::tui::widgets::TextInputEvent }, handle)
        }
        "AutocompleteField" => {
            let options = if let Some(expr) = options_expr {
                let tokens: proc_macro2::TokenStream = expr.parse().unwrap();
                quote! { &#tokens }
            } else {
                quote! { &[] }
            };
            let handle = quote! {
                self.#field_name.handle_event::<()>(event.clone(), #options);
            };
            (quote! { crate::tui::widgets::AutocompleteEvent }, handle)
        }
        "SelectField" => {
            let options = if let Some(expr) = options_expr {
                let tokens: proc_macro2::TokenStream = expr.parse().unwrap();
                quote! { &#tokens }
            } else {
                quote! { &[] }
            };
            let handle = quote! {
                self.#field_name.handle_event::<()>(event.clone(), #options);
            };
            (quote! { crate::tui::widgets::SelectEvent }, handle)
        }
        // Add more field types as needed
        _ => {
            // Unknown field type - generate a compile error
            let msg = format!("Unsupported widget field type: {}", type_name);
            let handle = quote! {
                compile_error!(#msg);
            };
            (quote! { () }, handle)
        }
    }
}

/// Extract simple type name from Type (e.g., "TextInputField" from TextInputField)
fn extract_type_name(ty: &Type) -> String {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident.to_string();
    }
    "Unknown".to_string()
}
