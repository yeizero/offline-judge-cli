mod catalog;
mod generate;
mod input;
mod lower;
mod relay;

use proc_macro::TokenStream;

#[proc_macro]
pub fn catalog(input: TokenStream) -> TokenStream {
    syn::parse::<relay::CatalogInvocation>(input)
        .and_then(|input| match input {
            relay::CatalogInvocation::Catalog(input) => generate::expand_catalog(&input),
            relay::CatalogInvocation::LocaleRelay(input) => generate::expand_locale_relay(&input),
            relay::CatalogInvocation::LocaleWithSchema(input) => generate::expand_locale(&input),
        })
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro]
pub fn define_i18n(input: TokenStream) -> TokenStream {
    syn::parse::<input::AggregatorInput>(input)
        .map_or_else(syn::Error::into_compile_error, |input| {
            generate::expand_define(&input)
        })
        .into()
}
