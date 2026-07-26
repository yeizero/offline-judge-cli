use std::collections::HashSet;

use syn::{
    Expr, Ident, Path, Result, Token, Type,
    parse::{Parse, ParseStream},
};

#[derive(Debug)]
pub struct CatalogInput {
    pub kind: CatalogKind,
    pub messages: Vec<Message>,
}

#[derive(Debug)]
pub enum CatalogKind {
    Default,
    Schema(Path),
}

#[derive(Debug)]
pub struct Message {
    pub key: Ident,
    pub has_argument_braces: bool,
    pub has_rest: bool,
    pub arguments: Vec<MessageArgument>,
    pub expression: Expr,
}

#[derive(Debug)]
pub struct MessageArgument {
    pub name: Ident,
    pub ty: Option<Type>,
}

impl Parse for CatalogInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let header = input.parse::<Ident>()?;
        let kind = if header == "default" {
            CatalogKind::Default
        } else if header == "schema" {
            input.parse::<Token![:]>()?;
            CatalogKind::Schema(input.parse()?)
        } else {
            return Err(syn::Error::new(
                header.span(),
                "expected `default` or `schema` catalog header",
            ));
        };
        input.parse::<Token![;]>()?;

        let mut messages = Vec::new();
        let mut keys = HashSet::new();
        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            if !keys.insert(key.to_string()) {
                return Err(syn::Error::new(
                    key.span(),
                    format!("message `{key}` has a duplicate catalog key"),
                ));
            }

            let has_argument_braces = input.peek(syn::token::Brace);
            let (arguments, has_rest) =
                parse_arguments(input, &key, matches!(kind, CatalogKind::Schema(_)))?;

            input.parse::<Token![=]>()?;
            let expression = input.parse::<Expr>()?;
            input.parse::<Token![;]>()?;
            messages.push(Message {
                key,
                has_argument_braces,
                has_rest,
                arguments,
                expression,
            });
        }

        Ok(Self { kind, messages })
    }
}

fn parse_arguments(
    input: ParseStream<'_>,
    key: &Ident,
    allow_rest: bool,
) -> Result<(Vec<MessageArgument>, bool)> {
    if !input.peek(syn::token::Brace) {
        return Ok((Vec::new(), false));
    }
    let content;
    syn::braced!(content in input);
    let mut arguments = Vec::new();
    let mut names = HashSet::new();
    while !content.is_empty() {
        if content.peek(Token![..]) {
            let rest = content.parse::<Token![..]>()?;
            if !allow_rest {
                return Err(syn::Error::new(
                    rest.spans[0],
                    "`..` is only allowed in locale catalog entries",
                ));
            }
            if !content.is_empty() {
                return Err(syn::Error::new(
                    rest.spans[0],
                    "`..` must be the final locale field",
                ));
            }
            return Ok((arguments, true));
        }
        let name = content.parse::<Ident>()?;
        if !names.insert(name.to_string()) {
            return Err(syn::Error::new(
                name.span(),
                format!("message `{key}` has duplicate argument `{name}`"),
            ));
        }
        let ty = if content.peek(Token![:]) {
            content.parse::<Token![:]>()?;
            Some(content.parse::<Type>()?)
        } else {
            None
        };
        arguments.push(MessageArgument { name, ty });
        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }
    Ok((arguments, false))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use quote::ToTokens as _;
    use syn::parse_str;

    use super::{CatalogInput, CatalogKind};

    #[test]
    fn parses_default_static_and_typed_and_untyped_dynamic_entries() {
        // Catches retaining inferred placeholders or a string-only RHS grammar.
        let catalog = parse_str::<CatalogInput>(
            r#"
                default;
                About = "Evaluator";
                Progress { disabled: bool, percent } =
                    if disabled { "Disabled" } else { "{percent} / 100" };
            "#,
        )
        .expect("native default catalog should parse");

        assert!(matches!(catalog.kind, CatalogKind::Default));
        assert_eq!(catalog.messages.len(), 2);
        assert_eq!(catalog.messages[0].key, "About");
        assert!(catalog.messages[0].arguments.is_empty());
        assert!(matches!(catalog.messages[0].expression, syn::Expr::Lit(_)));
        assert_eq!(catalog.messages[1].key, "Progress");
        assert_eq!(catalog.messages[1].arguments.len(), 2);
        assert_eq!(catalog.messages[1].arguments[0].name, "disabled");
        assert_eq!(
            catalog.messages[1].arguments[0]
                .ty
                .as_ref()
                .expect("disabled should be typed")
                .to_token_stream()
                .to_string(),
            "bool"
        );
        assert_eq!(catalog.messages[1].arguments[1].name, "percent");
        assert!(catalog.messages[1].arguments[1].ty.is_none());
        assert!(matches!(catalog.messages[1].expression, syn::Expr::If(_)));
        assert!(parse_str::<CatalogInput>("fallback; About = \"About\";").is_err());
    }

    #[test]
    fn parses_schema_path_and_native_match_expression() {
        // Catches reducing a schema declaration or match RHS to opaque text.
        let catalog = parse_str::<CatalogInput>(
            r#"
                schema: super::en_us;
                Count { count: usize } = match count {
                    0 => "none",
                    n => "{n}",
                };
            "#,
        )
        .expect("native locale catalog should parse");

        let CatalogKind::Schema(path) = catalog.kind else {
            panic!("expected schema catalog");
        };
        assert_eq!(path.to_token_stream().to_string(), "super :: en_us");
        assert!(matches!(
            catalog.messages[0].expression,
            syn::Expr::Match(_)
        ));
    }

    #[test]
    fn preserves_braces_and_final_locale_rest_marker() {
        let default_catalog = parse_str::<CatalogInput>(
            r#"
                default;
                Heartbeat {} = "alive";
            "#,
        )
        .expect("zero-field default message should parse");
        let locale = parse_str::<CatalogInput>(
            r#"
                schema: super::en_us;
                Count { count, .. } = "{count}";
                GenericNotice { .. } = "notice";
            "#,
        )
        .expect("locale rest marker should parse");

        assert!(default_catalog.messages[0].has_argument_braces);
        assert!(!default_catalog.messages[0].has_rest);
        assert!(locale.messages[0].has_argument_braces);
        assert!(locale.messages[0].has_rest);
        assert!(locale.messages[1].has_argument_braces);
        assert!(locale.messages[1].arguments.is_empty());
        assert!(locale.messages[1].has_rest);
    }

    #[test]
    fn rejects_invalid_rest_marker_placement() {
        for input in [
            r#"default; Count { count, .. } = "{count}";"#,
            r#"schema: super::en_us; Count { .., count } = "{count}";"#,
            r#"schema: super::en_us; Count { count, .., .. } = "{count}";"#,
        ] {
            let error = parse_str::<CatalogInput>(input)
                .expect_err("invalid rest marker placement must fail");
            let message = error.to_string();
            assert!(message.contains(".."));
        }
    }

    #[test]
    fn rejects_duplicate_keys() {
        // Catches generating two implementations for one schema item.
        let error = parse_str::<CatalogInput>(
            r#"
                default;
                About = "first";
                About = "second";
            "#,
        )
        .expect_err("duplicate keys must fail");

        let message = error.to_string();
        assert!(message.contains("About"));
        assert!(message.contains("duplicate"));
    }

    #[test]
    fn rejects_duplicate_arguments() {
        // Catches generating an ambiguous message value field.
        let error = parse_str::<CatalogInput>(
            r#"
                default;
                Broken { value, value: usize } = "{value}";
            "#,
        )
        .expect_err("duplicate arguments must fail");

        let message = error.to_string();
        assert!(message.contains("value"));
        assert!(message.contains("duplicate"));
    }

    #[test]
    fn rejects_malformed_header_and_entry() {
        // Catches accepting a catalog without a role or without an RHS.
        let header = parse_str::<CatalogInput>(r#"unknown; About = "x";"#)
            .expect_err("unknown header must fail")
            .to_string();
        let entry = parse_str::<CatalogInput>("default; About;")
            .expect_err("entry without equals must fail")
            .to_string();

        assert!(header.contains("default") || header.contains("schema"));
        assert!(entry.contains('='));
    }
}
