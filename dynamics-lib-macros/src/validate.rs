use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

use crate::utils::{get_attr_string, has_attr_flag};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let Data::Struct(data_struct) = &input.data else {
        return syn::Error::new_spanned(name, "Validate can only be derived for structs")
            .to_compile_error()
            .into();
    };

    let Fields::Named(fields) = &data_struct.fields else {
        return syn::Error::new_spanned(name, "Validate requires named fields")
            .to_compile_error()
            .into();
    };

    let mut validations = Vec::new();

    for field in fields.named.iter() {
        let field_name = field.ident.as_ref().unwrap();

        for attr in field.attrs.iter() {
            if !attr.path().is_ident("validate") {
                continue;
            }

            let message = get_attr_string(attr, "message")
                .unwrap_or_else(|| format!("Validation failed for {}", field_name));

            // Check validation type
            if has_attr_flag(attr, "required") {
                validations.push(quote! {
                    if self.#field_name.value().is_none() {
                        return Err(#message.to_string());
                    }
                });
            } else if has_attr_flag(attr, "not_empty") {
                validations.push(quote! {
                    if self.#field_name.value().trim().is_empty() {
                        return Err(#message.to_string());
                    }
                });
            } else if let Some(custom_fn) = get_attr_string(attr, "custom") {
                let fn_ident = syn::Ident::new(&custom_fn, field_name.span());
                validations.push(quote! {
                    self.#fn_ident().map_err(|_| #message.to_string())?;
                });
            }
        }
    }

    let expanded = quote! {
        impl #name {
            pub fn validate(&self) -> Result<(), String> {
                #(#validations)*
                Ok(())
            }
        }
    };

    TokenStream::from(expanded)
}
