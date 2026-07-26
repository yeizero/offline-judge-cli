use std::collections::HashSet;

use syn::{
    Ident, Path, Result, Token, Visibility,
    parse::{Parse, ParseStream},
};

#[derive(Debug)]
pub struct AggregatorInput {
    pub visibility: Visibility,
    pub locale: Ident,
    pub current_locale: Path,
    pub schema: Path,
    pub fallback: CatalogInput,
    pub locales: Vec<CatalogInput>,
}

#[derive(Debug)]
pub struct CatalogInput {
    pub variant: Ident,
    pub module: Path,
}

impl Parse for AggregatorInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        parse_label(input, "locale")?;
        input.parse::<Token![:]>()?;
        let visibility = input.parse()?;
        let locale = input.parse()?;
        input.parse::<Token![;]>()?;

        parse_label(input, "current_locale")?;
        input.parse::<Token![:]>()?;
        let current_locale = input.parse()?;
        input.parse::<Token![;]>()?;

        parse_label(input, "schema")?;
        input.parse::<Token![:]>()?;
        let schema = input.parse()?;
        input.parse::<Token![;]>()?;

        let mut fallback = None;
        let mut locales = Vec::new();
        let mut variants = HashSet::new();
        while !input.is_empty() {
            let kind = input.parse::<Ident>()?;
            let variant = input.parse::<Ident>()?;
            if !variants.insert(variant.to_string()) {
                return Err(syn::Error::new(
                    variant.span(),
                    format!("duplicate locale variant `{variant}`"),
                ));
            }
            input.parse::<Token![:]>()?;
            let module = input.parse::<Path>()?;
            input.parse::<Token![;]>()?;
            let locale = CatalogInput { variant, module };

            match kind.to_string().as_str() {
                "fallback" if fallback.is_none() => fallback = Some(locale),
                "fallback" => {
                    return Err(syn::Error::new(
                        kind.span(),
                        "only one fallback catalog may be declared",
                    ));
                }
                "locale" => locales.push(locale),
                _ => {
                    return Err(syn::Error::new(
                        kind.span(),
                        "expected `fallback` or `locale`",
                    ));
                }
            }
        }

        let fallback = fallback.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "a fallback catalog is required",
            )
        })?;
        if locales.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "at least one non-fallback locale is required",
            ));
        }

        Ok(Self {
            visibility,
            locale,
            current_locale,
            schema,
            fallback,
            locales,
        })
    }
}

fn parse_label(input: ParseStream<'_>, expected: &str) -> Result<()> {
    let label = input.parse::<Ident>()?;
    if label == expected {
        Ok(())
    } else {
        Err(syn::Error::new(
            label.span(),
            format!("expected `{expected}`"),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use quote::ToTokens as _;
    use syn::parse_str;

    use super::AggregatorInput;

    const DECLARATION: &str = r"
        locale: pub Locale;
        current_locale: current_locale;
        schema: en_us;
        fallback EnUs: en_us;
        locale ZhTw: zh_tw;
    ";

    #[test]
    fn parses_generated_locale_schema_and_catalog_types() {
        // Catches retaining a consumer locale type or file-path catalog inputs.
        let input =
            parse_str::<AggregatorInput>(DECLARATION).expect("native aggregator should parse");

        assert_eq!(input.visibility.to_token_stream().to_string(), "pub");
        assert_eq!(input.locale, "Locale");
        assert_eq!(
            input.current_locale.to_token_stream().to_string(),
            "current_locale"
        );
        assert_eq!(input.schema.to_token_stream().to_string(), "en_us");
        assert_eq!(input.fallback.variant, "EnUs");
        assert_eq!(input.fallback.module.to_token_stream().to_string(), "en_us");
        assert_eq!(input.locales.len(), 1);
        assert_eq!(input.locales[0].variant, "ZhTw");
        assert_eq!(
            input.locales[0].module.to_token_stream().to_string(),
            "zh_tw"
        );
    }

    #[test]
    fn rejects_missing_fallback_and_duplicate_variants() {
        // Catches generating an ungrounded router or duplicate locale match arms.
        let missing = parse_str::<AggregatorInput>(
            r"
                locale: Locale;
                current_locale: current_locale;
                schema: en_us;
                locale ZhTw: zh_tw;
            ",
        )
        .expect_err("missing fallback must fail")
        .to_string();
        let duplicate = parse_str::<AggregatorInput>(
            r"
                locale: Locale;
                current_locale: current_locale;
                schema: en_us;
                fallback EnUs: en_us;
                locale EnUs: zh_tw;
            ",
        )
        .expect_err("duplicate variants must fail")
        .to_string();

        assert!(missing.contains("fallback"));
        assert!(duplicate.contains("EnUs"));
    }
}
