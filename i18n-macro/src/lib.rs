mod catalog;
mod generate;
mod input;
mod lower;

use proc_macro::TokenStream;

#[proc_macro]
pub fn catalog(input: TokenStream) -> TokenStream {
    syn::parse::<catalog::CatalogInput>(input)
        .and_then(|input| generate::expand_catalog(&input))
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
