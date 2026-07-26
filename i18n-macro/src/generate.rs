use std::collections::HashSet;

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{ToTokens as _, quote, quote_spanned};
use syn::{Ident, Result, ext::IdentExt as _, spanned::Spanned as _};

use crate::{
    catalog::{CatalogInput, CatalogKind, Message, MessageArgument},
    input::AggregatorInput,
    lower::lower_message_expression,
    relay::{LocaleRelayInput, LocaleWithSchemaInput, MessageSchema, validate_locale_schema},
};

pub fn expand_catalog(input: &CatalogInput) -> Result<TokenStream> {
    match &input.kind {
        CatalogKind::Default => expand_default(input),
        CatalogKind::Schema(path) => {
            let _ = path;
            unreachable!("authored locale catalogs parse as LocaleRelayInput")
        }
    }
}

pub fn expand_locale_relay(input: &LocaleRelayInput) -> Result<TokenStream> {
    let schema = &input.schema_path;
    let fallback_path = input.fallback_path.as_ref().unwrap_or(schema);
    let locale_tokens = &input.locale_tokens;
    let callback = catalog_macro_path()?;

    Ok(quote! {
        #schema::__i18n_catalog::apply_locale! {
            callback: #callback;
            schema_path: #schema;
            fallback_path: #fallback_path;
            locale {
                #locale_tokens
            }
        }
    })
}

pub fn expand_locale(input: &LocaleWithSchemaInput) -> Result<TokenStream> {
    expand_partial_with_schema(
        &input.locale_messages,
        &input.schema_path,
        &input.fallback_path,
        &input.schema_messages,
    )
}

pub fn expand_define(input: &AggregatorInput) -> TokenStream {
    let visibility = &input.visibility;
    let locale = &input.locale;
    let formatter = generated_ident("formatter");
    let generated_locale = generated_ident("Locale");
    let message = generated_ident("M");
    let current_locale = path_from_generated_module(&input.current_locale);
    let schema_module = path_from_generated_module(&input.schema);
    let schema = quote!(#schema_module::__i18n_schema);
    let protocol = quote!(#schema_module::__i18n_catalog);
    let variants = std::iter::once(&input.default_catalog)
        .chain(&input.locales)
        .map(|catalog| &catalog.variant)
        .collect::<Vec<_>>();
    let catalogs = std::iter::once(&input.default_catalog)
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
            #generated_locale::#variant =>
                <#message as #protocol::StaticMessage<#catalog>>::VALUE
        }
    });
    let dynamic_routes = variants.iter().zip(&catalogs).map(|(variant, catalog)| {
        quote! {
            #generated_locale::#variant =>
                <#message as #protocol::DynamicMessage<#catalog>>::render(
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
            use super::#locale as #generated_locale;
            pub use #schema as schema;

            #[inline]
            pub fn current_locale() -> #generated_locale {
                #current_locale()
            }

            #[inline]
            pub const fn static_for<#message>(
                locale: #generated_locale,
            ) -> &'static str
            where
                #message: #(#static_bounds)+*,
            {
                match locale {
                    #(#static_routes,)*
                }
            }

            pub struct Localized<#message> {
                pub locale: #generated_locale,
                pub message: #message,
            }

            impl<#message> ::core::fmt::Display for Localized<#message>
            where
                #message: #(#dynamic_bounds)+*,
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
fn expand_default(input: &CatalogInput) -> Result<TokenStream> {
    let schema = quote!(__i18n_schema);
    let formatter = generated_ident("formatter");
    let static_messages = input
        .messages
        .iter()
        .filter(|message| !message.has_argument_braces)
        .collect::<Vec<_>>();
    let dynamic_messages = input
        .messages
        .iter()
        .filter(|message| message.has_argument_braces)
        .collect::<Vec<_>>();

    let static_types = static_messages.iter().map(|message| {
        let key = &message.key;
        quote! {
            pub struct #key;
        }
    });
    let dynamic_types = dynamic_messages.iter().copied().map(dynamic_type);
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
    let default_static_items = static_messages.iter().map(|message| {
        let name = static_name(&message.key);
        let expression = &message.expression;
        quote!(const #name: &'static str = #expression;)
    });
    let default_dynamic_items = dynamic_messages
        .iter()
        .copied()
        .map(|message| dynamic_override(message, &schema, &formatter))
        .collect::<Result<Vec<_>>>()?;
    let copy_helper = typed_copy_helper(&input.messages);
    let static_mappings = static_messages.iter().map(|message| {
        let key = &message.key;
        let name = static_name(key);
        quote! {
            impl<C: CatalogImpl> StaticMessage<C>
                for __i18n_schema::#key
            {
                const VALUE: &'static str = C::#name;
            }
        }
    });
    let dynamic_mappings = dynamic_messages.iter().copied().map(|message| {
        let key = &message.key;
        let shape = message_shape(&message.arguments);
        let parameters = &shape.parameters;
        let message_type = shape.type_tokens_with_schema(key, &schema);
        let implementation_generics = if parameters.is_empty() {
            quote!(<C>)
        } else {
            quote!(<C, #(#parameters),*>)
        };
        quote! {
            impl #implementation_generics DynamicMessage<C> for #message_type
            where
                C: CatalogImpl,
                #(#parameters: ::core::fmt::Display,)*
            {
                fn render(
                    &self,
                    #formatter: &mut ::core::fmt::Formatter<'_>,
                ) -> ::core::fmt::Result {
                    C::#key(
                        self,
                        #formatter,
                    )
                }
            }
        }
    });
    let relay_schema = input.messages.iter().map(schema_manifest_message);

    Ok(quote! {
        #[doc(hidden)]
        pub mod __i18n_schema {
            #[allow(unused_imports)]
            use super::*;

            #(#static_types)*
            #(#dynamic_types)*
        }

        #[doc(hidden)]
        pub mod __i18n_catalog {
            #[allow(unused_imports)]
            use super::*;
            use super::__i18n_schema;

            pub struct FallbackEnd;

            pub trait FallbackGraph {
                type Parent;
            }

            trait CompleteCatalog {}

            impl CompleteCatalog for FallbackEnd {}

            impl<T> CompleteCatalog for T
            where
                T: CatalogImpl + FallbackGraph,
                T::Parent: CompleteCatalog,
            {}

            #[doc(hidden)]
            #[allow(private_bounds)]
            pub fn assert_complete_catalog<T: CompleteCatalog>() {}

            #[allow(private_bounds)]
            #[allow(non_snake_case, non_upper_case_globals)]
            pub trait CatalogImpl: Sized {
                type Fallback: CatalogImpl + CompleteCatalog;

                #(#trait_static_items)*
                #(#trait_dynamic_items)*
            }

            pub struct Catalog;

            #copy_helper

            #[allow(non_snake_case, non_upper_case_globals)]
            impl CatalogImpl for Catalog {
                type Fallback = Self;

                #(#default_static_items)*
                #(#default_dynamic_items)*
            }

            impl FallbackGraph for Catalog {
                type Parent = FallbackEnd;
            }

            macro_rules! apply_locale {
                (
                    callback: $callback:path;
                    schema_path: $schema:path;
                    fallback_path: $fallback_path:path;
                    locale { $($locale:tt)* }
                ) => {
                    $callback! {
                        @locale_with_schema;
                        schema_path: $schema;
                        fallback_path: $fallback_path;
                        schema {
                            #(#relay_schema)*
                        }
                        locale { $($locale)* }
                    }
                };
            }

            pub(crate) use apply_locale;

            pub trait StaticMessage<C: CatalogImpl> {
                const VALUE: &'static str;
            }

            #(#static_mappings)*

            pub trait DynamicMessage<C: CatalogImpl> {
                fn render(
                    &self,
                    #formatter: &mut ::core::fmt::Formatter<'_>,
                ) -> ::core::fmt::Result;
            }

            #(#dynamic_mappings)*
        }
    })
}

fn expand_partial_with_schema(
    locale_messages: &[Message],
    schema: &syn::Path,
    fallback_path: &syn::Path,
    schema_messages: &[MessageSchema],
) -> Result<TokenStream> {
    let validated_schema_messages = validate_locale_schema(schema_messages, locale_messages)?;
    let fallback_path_span = fallback_path.span();
    let schema = path_from_generated_module(schema);
    let fallback_path = path_from_generated_module(fallback_path);
    let schema_types = quote!(#schema::__i18n_schema);
    let protocol = quote!(#schema::__i18n_catalog);
    let fallback_protocol = quote!(#fallback_path::__i18n_catalog);
    let fallback_catalog = quote_spanned!(fallback_path_span=> #fallback_protocol::Catalog);
    let local_proof = quote_spanned!(fallback_path_span=>
        const _: fn() = #protocol::assert_complete_catalog::<Catalog>;
    );
    let formatter = generated_ident("formatter");
    let copy_helper = typed_copy_helper(locale_messages);
    let mut static_items = Vec::new();
    let mut dynamic_items = Vec::new();

    for (message, schema_message) in locale_messages.iter().zip(validated_schema_messages) {
        if schema_message.is_dynamic {
            dynamic_items.push(dynamic_override_with_schema(
                message,
                schema_message,
                &schema_types,
                &formatter,
            )?);
        } else {
            static_items.push(static_override(message));
        }
    }

    Ok(quote! {
        #[doc(hidden)]
        pub mod __i18n_catalog {
            #[allow(unused_imports)]
            use super::*;

            pub struct Catalog;

            #copy_helper

            #[allow(non_snake_case, non_upper_case_globals)]
            impl #protocol::CatalogImpl for Catalog {
                type Fallback = #fallback_catalog;

                #(#static_items)*
                #(#dynamic_items)*
            }

            impl #protocol::FallbackGraph for Catalog {
                type Parent = #fallback_catalog;
            }

            #local_proof
        }
    })
}

fn schema_manifest_message(message: &Message) -> TokenStream {
    let key = &message.key;
    if message.has_argument_braces {
        let arguments = message.arguments.iter().map(|argument| {
            let name = &argument.name;
            if let Some(ty) = &argument.ty {
                quote!(#name: #ty)
            } else {
                quote!(#name)
            }
        });
        quote!(dynamic #key { #(#arguments),* };)
    } else {
        quote!(static #key;)
    }
}

fn catalog_macro_path() -> Result<TokenStream> {
    match proc_macro_crate::crate_name(env!("CARGO_PKG_NAME")) {
        Ok(proc_macro_crate::FoundCrate::Itself) => Ok(quote!(crate::catalog)),
        Ok(proc_macro_crate::FoundCrate::Name(name)) => {
            let crate_name = Ident::new(&name, Span::call_site());
            Ok(quote!(#crate_name::catalog))
        }
        Err(error) => Err(syn::Error::new(
            Span::call_site(),
            format!("could not resolve the catalog macro crate: {error}"),
        )),
    }
}

fn static_override(message: &Message) -> TokenStream {
    let name = static_name(&message.key);
    let expression = &message.expression;
    quote!(const #name: &'static str = #expression;)
}

fn dynamic_type(message: &Message) -> TokenStream {
    let key = &message.key;
    let shape = message_shape(&message.arguments);
    let declaration_generics = shape.declaration_generics();
    let fields = shape.fields.iter().map(|(name, ty)| quote!(pub #name: #ty));

    quote! {
        #[allow(clippy::pub_underscore_fields)]
        pub struct #key #declaration_generics {
            #(#fields,)*
        }
    }
}

fn dynamic_trait_item(message: &Message, schema: &TokenStream, formatter: &Ident) -> TokenStream {
    let key = &message.key;
    let shape = message_shape(&message.arguments);
    let method_generics = shape.method_generics();
    let message_type = shape.type_tokens_with_schema(key, schema);
    let message_value = generated_ident("message");

    quote! {
        fn #key #method_generics (
            #message_value: &#message_type,
            #formatter: &mut ::core::fmt::Formatter<'_>,
        ) -> ::core::fmt::Result {
            <Self::Fallback as CatalogImpl>::#key(
                #message_value,
                #formatter,
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
    let shape = message_shape(&message.arguments);
    let method_generics = shape.method_generics();
    let message_type = shape.type_tokens_with_schema(key, schema);
    let message_value = generated_ident("message");
    let bindings = message.arguments.iter().map(|argument| {
        let name = &argument.name;
        if let Some(ty) = &argument.ty {
            quote! {
                let #name: #ty = Catalog::copy_value(&#message_value.#name);
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
        ) -> ::core::fmt::Result {
            #(#bindings)*
            #body
        }
    })
}

fn dynamic_override_with_schema(
    message: &Message,
    schema_message: &MessageSchema,
    schema: &TokenStream,
    formatter: &Ident,
) -> Result<TokenStream> {
    let key = &message.key;
    let shape = message_shape(&schema_message.arguments);
    let method_generics = shape.method_generics();
    let message_type = shape.type_tokens_with_schema(key, schema);
    let message_value = generated_ident("message");
    let bindings = message.arguments.iter().map(|argument| {
        let name = &argument.name;
        if let Some(ty) = &argument.ty {
            quote! {
                let #name: #ty = Catalog::copy_value(&#message_value.#name);
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
        ) -> ::core::fmt::Result {
            #(#bindings)*
            #body
        }
    })
}

fn typed_copy_helper(messages: &[Message]) -> TokenStream {
    if messages
        .iter()
        .flat_map(|message| &message.arguments)
        .any(|argument| argument.ty.is_some())
    {
        quote! {
            impl Catalog {
                fn copy_value<T: ::core::marker::Copy>(value: &T) -> T {
                    *value
                }
            }
        }
    } else {
        TokenStream::new()
    }
}

fn static_name(key: &Ident) -> Ident {
    key.clone()
}

fn generated_ident(name: &str) -> Ident {
    Ident::new(name, Span::mixed_site())
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

fn message_shape(arguments: &[MessageArgument]) -> MessageShape {
    let reserved_type_identifiers = arguments
        .iter()
        .filter_map(|argument| argument.ty.as_ref())
        .flat_map(type_identifier_names)
        .collect::<HashSet<_>>();
    let mut parameters = Vec::new();
    let fields = canonical_arguments(arguments)
        .into_iter()
        .enumerate()
        .map(|(index, argument)| {
            (
                argument.name.clone(),
                argument_type(index, argument, &reserved_type_identifiers, &mut parameters),
            )
        })
        .collect();
    MessageShape { parameters, fields }
}

fn canonical_arguments(arguments: &[MessageArgument]) -> Vec<&MessageArgument> {
    let mut arguments = arguments.iter().collect::<Vec<_>>();
    arguments.sort_by_key(|argument| argument.name.to_string());
    arguments
}

fn argument_type(
    index: usize,
    argument: &MessageArgument,
    reserved_type_identifiers: &HashSet<String>,
    parameters: &mut Vec<Ident>,
) -> TokenStream {
    if let Some(ty) = &argument.ty {
        quote!(#ty)
    } else {
        let mut suffix = index;
        let parameter = loop {
            let candidate = format!("A{suffix}");
            if !reserved_type_identifiers.contains(&candidate)
                && parameters
                    .iter()
                    .all(|parameter| parameter != candidate.as_str())
            {
                break Ident::new(&candidate, Span::mixed_site());
            }
            suffix += 1;
        };
        parameters.push(parameter.clone());
        quote!(#parameter)
    }
}

fn type_identifier_names(ty: &syn::Type) -> Vec<String> {
    fn collect(tokens: TokenStream, identifiers: &mut Vec<String>) {
        for token in tokens {
            match token {
                TokenTree::Ident(identifier) => identifiers.push(identifier.unraw().to_string()),
                TokenTree::Group(group) => collect(group.stream(), identifiers),
                TokenTree::Literal(_) | TokenTree::Punct(_) => {}
            }
        }
    }

    let mut identifiers = Vec::new();
    collect(ty.to_token_stream(), &mut identifiers);
    identifiers
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
