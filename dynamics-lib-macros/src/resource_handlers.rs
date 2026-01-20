use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type, parse_macro_input};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let Data::Struct(data_struct) = &input.data else {
        return syn::Error::new_spanned(struct_name, "ResourceHandlers only works on structs")
            .to_compile_error()
            .into();
    };

    let Fields::Named(fields) = &data_struct.fields else {
        return syn::Error::new_spanned(struct_name, "ResourceHandlers requires named fields")
            .to_compile_error()
            .into();
    };

    let mut methods = Vec::new();

    for field in fields.named.iter() {
        let field_name = field.ident.as_ref().unwrap();

        // Check if field has #[resource(...)] attribute
        for attr in field.attrs.iter() {
            if !attr.path().is_ident("resource") {
                continue;
            }

            let mut loader_fn = None;
            let mut on_complete_msg = None;

            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("loader") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    loader_fn = Some(lit.value());
                } else if meta.path.is_ident("on_complete") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    on_complete_msg = Some(lit.value());
                }
                Ok(())
            });

            let Some(loader) = loader_fn else {
                continue;
            };

            // Extract the inner type from Resource<T, E> or Resource<T>
            let Type::Path(type_path) = &field.ty else {
                continue;
            };
            let last_segment = type_path.path.segments.last().unwrap();

            if last_segment.ident != "Resource" {
                continue;
            }

            let PathArguments::AngleBracketed(args) = &last_segment.arguments else {
                continue;
            };

            // Get the T from Resource<T, E> or Resource<T>
            let GenericArgument::Type(inner_type) = &args.args[0] else {
                continue;
            };

            // Generate method names based on field name
            let loader_ident = format_ident!("{}", loader);
            let load_method = format_ident!("load_{}", field_name);
            let handle_method = format_ident!("handle_{}_loaded", field_name);

            // Convert field_name to PascalCase for Msg variant
            let msg_variant = format_ident!(
                "{}Loaded",
                field_name
                    .to_string()
                    .split('_')
                    .map(|s| {
                        let mut c = s.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        }
                    })
                    .collect::<String>()
            );

            // Generate load method
            let load_impl = quote! {
                fn #load_method(&mut self) -> Command<Msg> {
                    self.#field_name = Resource::Loading;
                    Command::perform(#loader_ident(), Msg::#msg_variant)
                }
            };

            // Generate handle method with optional on_complete
            let handle_impl = if let Some(on_complete) = on_complete_msg {
                let on_complete_ident = format_ident!("{}", on_complete);
                quote! {
                    fn #handle_method(&mut self, result: Result<#inner_type, String>) -> Command<Msg> {
                        self.#field_name = Resource::from_result(result);
                        if self.#field_name.is_success() {
                            Command::Perform(Box::pin(async move { Msg::#on_complete_ident }))
                        } else {
                            Command::None
                        }
                    }
                }
            } else {
                quote! {
                    fn #handle_method(&mut self, result: Result<#inner_type, String>) -> Command<Msg> {
                        self.#field_name = Resource::from_result(result);
                        Command::None
                    }
                }
            };

            methods.push(load_impl);
            methods.push(handle_impl);
        }
    }

    let expanded = quote! {
        impl #struct_name {
            #(#methods)*
        }
    };

    TokenStream::from(expanded)
}
