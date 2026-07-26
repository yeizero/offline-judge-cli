use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Ident, Result};

use crate::{
    catalog::{CatalogInput, CatalogKind, Message, MessageArgument},
    input::AggregatorInput,
    lower::lower_message_expression,
};

pub fn expand_catalog(input: &CatalogInput) -> Result<TokenStream> {
    match &input.kind {
        CatalogKind::Fallback => expand_fallback(input),
        CatalogKind::Schema(path) => expand_partial(input, path),
    }
}

pub fn expand_define(input: &AggregatorInput) -> TokenStream {
    let visibility = &input.visibility;
    let locale = &input.locale;
    let formatter = generated_ident("__i18n_formatter");
    let current_locale = path_from_generated_module(&input.current_locale);
    let schema_module = path_from_generated_module(&input.schema);
    let schema = quote!(#schema_module::__i18n_schema);
    let protocol = quote!(#schema_module::__i18n_catalog);
    let variants = std::iter::once(&input.fallback)
        .chain(&input.locales)
        .map(|catalog| &catalog.variant)
        .collect::<Vec<_>>();
    let catalogs = std::iter::once(&input.fallback)
        .chain(&input.locales)
        .map(|catalog| {
            let module = path_from_generated_module(&catalog.module);
            quote!(#module::__i18n_catalog::Catalog)
        })
        .collect::<Vec<_>>();
    let static_bounds = catalogs
        .iter()
        .map(|catalog| quote!(#protocol::StaticMessage<#catalog>));
    let dynamic_bounds = catalogs
        .iter()
        .map(|catalog| quote!(#protocol::DynamicMessage<#catalog>));
    let static_routes = variants.iter().zip(&catalogs).map(|(variant, catalog)| {
        quote! {
            __I18nLocale::#variant =>
                <__I18nMessage as #protocol::StaticMessage<#catalog>>::VALUE
        }
    });
    let dynamic_routes = variants.iter().zip(&catalogs).map(|(variant, catalog)| {
        quote! {
            __I18nLocale::#variant =>
                <__I18nMessage as #protocol::DynamicMessage<#catalog>>::render(
                    &self.message,
                    #formatter,
                )
        }
    });

    let caller_macros = caller_macros();

    quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #visibility enum #locale {
            #(#variants,)*
        }

        #[doc(hidden)]
        pub mod __i18n_generated {
            use super::#locale as __I18nLocale;
            #[doc(hidden)]
            pub use #schema as schema;

            #[inline]
            pub fn current_locale() -> __I18nLocale {
                #current_locale()
            }

            #[inline]
            pub const fn static_for<__I18nMessage>(
                locale: __I18nLocale,
            ) -> &'static str
            where
                __I18nMessage: #(#static_bounds)+*,
            {
                match locale {
                    #(#static_routes,)*
                }
            }

            #[doc(hidden)]
            pub struct Localized<__I18nMessage> {
                pub locale: __I18nLocale,
                pub message: __I18nMessage,
            }

            impl<__I18nMessage> ::core::fmt::Display for Localized<__I18nMessage>
            where
                __I18nMessage: #(#dynamic_bounds)+*,
            {
                fn fmt(
                    &self,
                    #formatter: &mut ::core::fmt::Formatter<'_>,
                ) -> ::core::fmt::Result {
                    match self.locale {
                        #(#dynamic_routes,)*
                    }
                }
            }
        }

        #caller_macros
    }
}

fn caller_macros() -> TokenStream {
    quote! {
        #[macro_export]
        macro_rules! tr_for {
            ($locale:expr, $message:ident { $($fields:tt)* }) => {
                $crate::__i18n_generated::Localized {
                    locale: $locale,
                    message: $crate::__i18n_generated::schema::$message { $($fields)* },
                }
            };
            ($locale:expr, $message:ident) => {
                $crate::__i18n_generated::static_for::<
                    $crate::__i18n_generated::schema::$message
                >($locale)
            };
        }

        #[macro_export]
        macro_rules! tr {
            ($message:ident { $($fields:tt)* }) => {
                $crate::__i18n_generated::Localized {
                    locale: $crate::__i18n_generated::current_locale(),
                    message: $crate::__i18n_generated::schema::$message { $($fields)* },
                }
            };
            ($message:ident) => {
                $crate::__i18n_generated::static_for::<
                    $crate::__i18n_generated::schema::$message
                >($crate::__i18n_generated::current_locale())
            };
        }
    }
}

#[allow(clippy::too_many_lines)]
fn expand_fallback(input: &CatalogInput) -> Result<TokenStream> {
    let schema = quote!(__i18n_schema);
    let formatter = generated_ident("__i18n_formatter");
    let static_messages = input
        .messages
        .iter()
        .filter(|message| message.arguments.is_empty())
        .collect::<Vec<_>>();
    let dynamic_messages = input
        .messages
        .iter()
        .filter(|message| !message.arguments.is_empty())
        .collect::<Vec<_>>();

    let static_types = static_messages.iter().map(|message| {
        let key = &message.key;
        quote! {
            #[doc(hidden)]
            pub struct #key;
        }
    });
    let dynamic_types = dynamic_messages.iter().copied().map(dynamic_type);
    let field_markers = dynamic_messages.iter().copied().flat_map(|message| {
        message.arguments.iter().map(|argument| {
            let marker = field_marker(&message.key, &argument.name);
            quote! {
                #[doc(hidden)]
                #[allow(non_camel_case_types)]
                pub struct #marker;
            }
        })
    });
    let trait_static_items = static_messages.iter().map(|message| {
        let name = static_name(&message.key);
        quote! {
            const #name: &'static str =
                <Self::Fallback as CatalogImpl>::#name;
        }
    });
    let trait_dynamic_items = dynamic_messages
        .iter()
        .copied()
        .map(|message| dynamic_trait_item(message, &schema, &formatter));
    let fallback_static_items = static_messages.iter().map(|message| {
        let name = static_name(&message.key);
        let expression = &message.expression;
        quote!(const #name: &'static str = #expression;)
    });
    let fallback_dynamic_items = dynamic_messages
        .iter()
        .copied()
        .map(|message| dynamic_override(message, &schema, &formatter))
        .collect::<Result<Vec<_>>>()?;
    let static_mappings = static_messages.iter().map(|message| {
        let key = &message.key;
        let name = static_name(key);
        quote! {
            impl<__I18nCatalog: CatalogImpl> StaticMessage<__I18nCatalog>
                for __i18n_schema::#key
            {
                const VALUE: &'static str = __I18nCatalog::#name;
            }
        }
    });
    let dynamic_mappings = dynamic_messages.iter().copied().map(|message| {
        let key = &message.key;
        let shape = message_shape(message);
        let parameters = &shape.parameters;
        let message_type = shape.type_tokens_with_schema(key, &schema);
        let implementation_generics = if parameters.is_empty() {
            quote!(<__I18nCatalog>)
        } else {
            quote!(<__I18nCatalog, #(#parameters),*>)
        };
        quote! {
            impl #implementation_generics DynamicMessage<__I18nCatalog> for #message_type
            where
                __I18nCatalog: CatalogImpl,
                #(#parameters: ::core::fmt::Display,)*
            {
                fn render(
                    &self,
                    #formatter: &mut ::core::fmt::Formatter<'_>,
                ) -> ::core::fmt::Result {
                    __I18nCatalog::#key(
                        self,
                        #formatter,
                        ::core::marker::PhantomData,
                    )
                }
            }
        }
    });

    Ok(quote! {
        #[doc(hidden)]
        pub mod __i18n_schema {
            #[allow(unused_imports)]
            use super::*;

            #(#static_types)*
            #(#dynamic_types)*
            #(#field_markers)*

            #[doc(hidden)]
            pub struct __I18nUntyped;
        }

        #[doc(hidden)]
        pub mod __i18n_catalog {
            #[allow(unused_imports)]
            use super::*;
            use super::__i18n_schema;

            #[doc(hidden)]
            trait CompleteCatalog: CatalogImpl {}

            #[doc(hidden)]
            #[allow(private_bounds)]
            #[allow(non_snake_case, non_upper_case_globals)]
            pub trait CatalogImpl: Sized {
                type Fallback: CompleteCatalog;
                #[doc(hidden)]
                const __I18N_GENERATED_CATALOG: ();

                #(#trait_static_items)*
                #(#trait_dynamic_items)*
            }

            #[doc(hidden)]
            pub struct Catalog;

            #[allow(non_snake_case, non_upper_case_globals)]
            impl CatalogImpl for Catalog {
                type Fallback = Self;
                const __I18N_GENERATED_CATALOG: () = ();

                #(#fallback_static_items)*
                #(#fallback_dynamic_items)*
            }

            impl CompleteCatalog for Catalog {}

            #[doc(hidden)]
            pub trait StaticMessage<__I18nCatalog: CatalogImpl> {
                const VALUE: &'static str;
            }

            #(#static_mappings)*

            #[doc(hidden)]
            pub trait DynamicMessage<__I18nCatalog: CatalogImpl> {
                fn render(
                    &self,
                    #formatter: &mut ::core::fmt::Formatter<'_>,
                ) -> ::core::fmt::Result;
            }

            #(#dynamic_mappings)*
        }
    })
}

fn expand_partial(input: &CatalogInput, schema: &syn::Path) -> Result<TokenStream> {
    let schema = path_from_generated_module(schema);
    let schema_types = quote!(#schema::__i18n_schema);
    let protocol = quote!(#schema::__i18n_catalog);
    let formatter = generated_ident("__i18n_formatter");
    let static_items = input
        .messages
        .iter()
        .filter(|message| message.arguments.is_empty())
        .map(|message| {
            let name = static_name(&message.key);
            let expression = &message.expression;
            quote!(const #name: &'static str = #expression;)
        });
    let dynamic_items = input
        .messages
        .iter()
        .filter(|message| !message.arguments.is_empty())
        .map(|message| dynamic_override(message, &schema_types, &formatter))
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        #[doc(hidden)]
        pub mod __i18n_catalog {
            #[allow(unused_imports)]
            use super::*;

            #[doc(hidden)]
            pub struct Catalog;

            #[allow(non_snake_case, non_upper_case_globals)]
            impl #protocol::CatalogImpl for Catalog {
                type Fallback = #protocol::Catalog;
                const __I18N_GENERATED_CATALOG: () = ();

                #(#static_items)*
                #(#dynamic_items)*
            }
        }
    })
}

fn dynamic_type(message: &Message) -> TokenStream {
    let key = &message.key;
    let shape = message_shape(message);
    let declaration_generics = shape.declaration_generics();
    let fields = shape.fields.iter().map(|(name, ty)| quote!(pub #name: #ty));

    quote! {
        #[doc(hidden)]
        #[allow(clippy::pub_underscore_fields)]
        pub struct #key #declaration_generics {
            #(#fields,)*
        }
    }
}

fn dynamic_trait_item(message: &Message, schema: &TokenStream, formatter: &Ident) -> TokenStream {
    let key = &message.key;
    let shape = message_shape(message);
    let method_generics = shape.method_generics();
    let message_type = shape.type_tokens_with_schema(key, schema);
    let signature = signature_type(message, schema);
    let message_value = generated_ident("__i18n_message");

    quote! {
        fn #key #method_generics (
            #message_value: &#message_type,
            #formatter: &mut ::core::fmt::Formatter<'_>,
            __i18n_signature: ::core::marker::PhantomData<#signature>,
        ) -> ::core::fmt::Result {
            <Self::Fallback as CatalogImpl>::#key(
                #message_value,
                #formatter,
                __i18n_signature,
            )
        }
    }
}

fn dynamic_override(
    message: &Message,
    schema: &TokenStream,
    formatter: &Ident,
) -> Result<TokenStream> {
    let key = &message.key;
    let shape = message_shape(message);
    let method_generics = shape.method_generics();
    let message_type = shape.type_tokens_with_schema(key, schema);
    let signature = signature_type(message, schema);
    let message_value = generated_ident("__i18n_message");
    let bindings = message.arguments.iter().map(|argument| {
        let name = &argument.name;
        if let Some(ty) = &argument.ty {
            quote! {
                let #name: #ty = {
                    fn __i18n_copy<__I18nValue: ::core::marker::Copy>(
                        value: &__I18nValue,
                    ) -> __I18nValue {
                        *value
                    }
                    __i18n_copy(&#message_value.#name)
                };
            }
        } else {
            quote!(let #name = &#message_value.#name;)
        }
    });
    let body = lower_message_expression(&message.expression, formatter)?;

    Ok(quote! {
        #[allow(unused_variables)]
        fn #key #method_generics (
            #message_value: &#message_type,
            #formatter: &mut ::core::fmt::Formatter<'_>,
            _: ::core::marker::PhantomData<#signature>,
        ) -> ::core::fmt::Result {
            #(#bindings)*
            #body
        }
    })
}

fn static_name(key: &Ident) -> Ident {
    format_ident!("__I18N_{key}", span = key.span())
}

fn generated_ident(name: &str) -> Ident {
    Ident::new(name, Span::mixed_site())
}

fn field_marker(key: &Ident, argument: &Ident) -> Ident {
    let key_length = key.to_string().len();
    let argument_length = argument.to_string().len();
    format_ident!(
        "__I18nField_K{key_length}_{key}_F{argument_length}_{argument}",
        span = argument.span(),
    )
}

fn signature_type(message: &Message, schema: &TokenStream) -> TokenStream {
    let elements = canonical_arguments(message).into_iter().map(|argument| {
        let marker = field_marker(&message.key, &argument.name);
        let marker = quote!(#schema::#marker);
        let ty = if let Some(ty) = &argument.ty {
            quote!(#ty)
        } else {
            quote!(#schema::__I18nUntyped)
        };
        quote!((#marker, #ty))
    });

    quote!((#(#elements,)*))
}

struct MessageShape {
    parameters: Vec<Ident>,
    fields: Vec<(Ident, TokenStream)>,
}

impl MessageShape {
    fn declaration_generics(&self) -> TokenStream {
        let parameters = &self.parameters;
        if parameters.is_empty() {
            TokenStream::new()
        } else {
            quote!(<#(#parameters),*>)
        }
    }

    fn method_generics(&self) -> TokenStream {
        let parameters = &self.parameters;
        if parameters.is_empty() {
            TokenStream::new()
        } else {
            quote!(<#(#parameters: ::core::fmt::Display),*>)
        }
    }

    fn type_tokens_with_schema(&self, key: &Ident, schema: &TokenStream) -> TokenStream {
        let parameters = &self.parameters;
        let path = quote!(#schema::#key);
        if parameters.is_empty() {
            path
        } else {
            quote!(#path<#(#parameters),*>)
        }
    }
}

fn message_shape(message: &Message) -> MessageShape {
    let mut parameters = Vec::new();
    let fields = canonical_arguments(message)
        .into_iter()
        .enumerate()
        .map(|(index, argument)| {
            (
                argument.name.clone(),
                argument_type(index, argument, &mut parameters),
            )
        })
        .collect();
    MessageShape { parameters, fields }
}

fn canonical_arguments(message: &Message) -> Vec<&MessageArgument> {
    let mut arguments = message.arguments.iter().collect::<Vec<_>>();
    arguments.sort_by_key(|argument| argument.name.to_string());
    arguments
}

fn argument_type(
    index: usize,
    argument: &MessageArgument,
    parameters: &mut Vec<Ident>,
) -> TokenStream {
    if let Some(ty) = &argument.ty {
        quote!(#ty)
    } else {
        let parameter = format_ident!("__I18nArgument{index}", span = argument.name.span());
        parameters.push(parameter.clone());
        quote!(#parameter)
    }
}

fn path_from_generated_module(path: &syn::Path) -> TokenStream {
    let starts_at_root = path.leading_colon.is_some()
        || path
            .segments
            .first()
            .is_some_and(|segment| segment.ident == "crate");
    if starts_at_root {
        quote!(#path)
    } else {
        quote!(super::#path)
    }
}

#[cfg(test)]
mod tests {
    use quote::format_ident;

    use super::field_marker;

    #[test]
    fn field_marker_identity_is_structurally_unambiguous() {
        // Catches separator-only concatenation aliasing distinct key/field pairs.
        assert_ne!(
            field_marker(&format_ident!("A_B"), &format_ident!("C")),
            field_marker(&format_ident!("A"), &format_ident!("B_C")),
        );
    }
}
