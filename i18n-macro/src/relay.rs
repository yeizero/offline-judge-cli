use std::collections::{HashMap, HashSet};

use proc_macro2::{TokenStream, TokenTree};
use syn::{
    Expr, Ident, Path, Result, Token, Type,
    parse::{Parse, ParseStream},
};

use crate::catalog::{CatalogInput, Message, MessageArgument};

pub enum CatalogInvocation {
    Catalog(CatalogInput),
    LocaleRelay(LocaleRelayInput),
    LocaleWithSchema(LocaleWithSchemaInput),
}

impl Parse for CatalogInvocation {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(Token![@]) {
            input.parse::<Token![@]>()?;
            let marker = input.parse::<Ident>()?;
            if marker != "locale_with_schema" {
                return Err(syn::Error::new(
                    marker.span(),
                    "expected `locale_with_schema`",
                ));
            }
            input.parse::<Token![;]>()?;
            LocaleWithSchemaInput::parse(input).map(Self::LocaleWithSchema)
        } else if is_authored_locale_relay(input)? {
            LocaleRelayInput::parse(input).map(Self::LocaleRelay)
        } else {
            CatalogInput::parse(input).map(Self::Catalog)
        }
    }
}

fn is_authored_locale_relay(input: ParseStream<'_>) -> Result<bool> {
    let fork = input.fork();
    if !fork.peek(Ident) {
        return Ok(false);
    }

    let header = fork.parse::<Ident>()?;
    Ok(header == "schema" && fork.peek(Token![:]))
}

pub struct LocaleRelayInput {
    pub schema_path: Path,
    pub fallback_path: Option<Path>,
    pub locale_tokens: TokenStream,
}

impl Parse for LocaleRelayInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let schema_label = input.parse::<Ident>()?;
        if schema_label != "schema" {
            return Err(syn::Error::new(schema_label.span(), "expected `schema`"));
        }
        input.parse::<Token![:]>()?;
        let schema_path = input.parse()?;
        input.parse::<Token![;]>()?;

        let fallback_path = if input.peek(Ident) {
            let fork = input.fork();
            let label = fork.parse::<Ident>()?;
            if label == "fallback" && fork.peek(Token![:]) {
                input.parse::<Ident>()?;
                input.parse::<Token![:]>()?;
                let path = input.parse()?;
                input.parse::<Token![;]>()?;
                Some(path)
            } else {
                None
            }
        } else {
            None
        };
        let locale_tokens = input.parse()?;
        if let Some(span) = late_fallback_header_span(&locale_tokens) {
            return Err(syn::Error::new(
                span,
                "`fallback:` must appear immediately after `schema:`",
            ));
        }

        Ok(Self {
            schema_path,
            fallback_path,
            locale_tokens,
        })
    }
}

fn late_fallback_header_span(tokens: &TokenStream) -> Option<proc_macro2::Span> {
    let mut token_trees = tokens.clone().into_iter().peekable();
    while let Some(token_tree) = token_trees.next() {
        if let TokenTree::Ident(ident) = token_tree
            && ident == "fallback"
            && matches!(token_trees.peek(), Some(TokenTree::Punct(punct)) if punct.as_char() == ':')
        {
            return Some(ident.span());
        }
    }
    None
}

pub struct LocaleWithSchemaInput {
    pub schema_path: Path,
    pub fallback_path: Path,
    pub schema_messages: Vec<MessageSchema>,
    pub locale_messages: Vec<Message>,
}

pub struct MessageSchema {
    pub key: Ident,
    pub is_dynamic: bool,
    pub arguments: Vec<MessageArgument>,
}

pub fn validate_locale_schema<'a>(
    schema_messages: &'a [MessageSchema],
    locale_messages: &[Message],
) -> Result<Vec<&'a MessageSchema>> {
    let schema_by_key = schema_messages
        .iter()
        .map(|message| (message.key.to_string(), message))
        .collect::<HashMap<_, _>>();
    let mut schema_matches = Vec::with_capacity(locale_messages.len());
    let mut errors = None;

    for locale_message in locale_messages {
        let Some(schema_message) = schema_by_key.get(&locale_message.key.to_string()) else {
            combine_error(
                &mut errors,
                syn::Error::new(
                    locale_message.key.span(),
                    format!(
                        "message `{}` does not exist in the default schema",
                        locale_message.key
                    ),
                ),
            );
            continue;
        };

        schema_matches.push(*schema_message);
        if schema_message.is_dynamic {
            validate_dynamic_locale_fields(locale_message, schema_message, &mut errors);
        } else if let Some(argument) = locale_message.arguments.first() {
            combine_error(
                &mut errors,
                syn::Error::new(
                    argument.name.span(),
                    format!(
                        "static default schema message `{}` does not accept locale fields",
                        locale_message.key
                    ),
                ),
            );
        } else if locale_message.has_argument_braces {
            combine_error(
                &mut errors,
                syn::Error::new(
                    locale_message.key.span(),
                    format!(
                        "static default schema message `{}` does not accept locale braces",
                        locale_message.key
                    ),
                ),
            );
        }
    }

    if let Some(errors) = errors {
        Err(errors)
    } else {
        Ok(schema_matches)
    }
}

fn validate_dynamic_locale_fields(
    locale_message: &Message,
    schema_message: &MessageSchema,
    errors: &mut Option<syn::Error>,
) {
    if !locale_message.has_argument_braces {
        combine_error(
            errors,
            syn::Error::new(
                locale_message.key.span(),
                format!(
                    "dynamic default schema message `{}` requires locale argument braces",
                    locale_message.key
                ),
            ),
        );
    }

    let mut has_unknown_field = false;
    for argument in &locale_message.arguments {
        if !schema_message
            .arguments
            .iter()
            .any(|schema_argument| schema_argument.name == argument.name)
        {
            has_unknown_field = true;
            combine_error(
                errors,
                unknown_locale_field_error(locale_message, argument, schema_message),
            );
        }
    }

    let omitted = schema_message
        .arguments
        .iter()
        .filter(|schema_argument| {
            !locale_message
                .arguments
                .iter()
                .any(|argument| argument.name == schema_argument.name)
        })
        .map(|argument| argument.name.to_string())
        .collect::<Vec<_>>();
    if locale_message.has_argument_braces
        && !has_unknown_field
        && !omitted.is_empty()
        && !locale_message.has_rest
    {
        combine_error(
            errors,
            syn::Error::new(
                locale_message.key.span(),
                format!(
                    "locale message `{}` omits default schema fields {}; add `..` to acknowledge them",
                    locale_message.key,
                    omitted.join(", ")
                ),
            ),
        );
    }
}

fn combine_error(errors: &mut Option<syn::Error>, error: syn::Error) {
    if let Some(errors) = errors {
        errors.combine(error);
    } else {
        *errors = Some(error);
    }
}

fn unknown_locale_field_error(
    locale_message: &Message,
    argument: &MessageArgument,
    schema_message: &MessageSchema,
) -> syn::Error {
    let field_names = schema_message
        .arguments
        .iter()
        .map(|schema_argument| schema_argument.name.to_string())
        .collect::<Vec<_>>();
    let suggestion = closest_field_name(&argument.name.to_string(), &field_names);
    let detail = if let Some(suggestion) = suggestion {
        format!("; did you mean `{suggestion}`?")
    } else if field_names.is_empty() {
        String::from("; this default schema message has no fields")
    } else {
        format!("; valid default schema fields: {}", field_names.join(", "))
    };

    syn::Error::new(
        argument.name.span(),
        format!(
            "locale field `{}` does not exist for dynamic default schema message `{}`{detail}",
            argument.name, locale_message.key
        ),
    )
}

fn closest_field_name<'a>(field: &str, candidates: &'a [String]) -> Option<&'a str> {
    let mut closest = None;
    let mut distance = usize::MAX;
    let mut is_unique = true;

    for candidate in candidates {
        let candidate_distance = edit_distance(field, candidate);
        if candidate_distance < distance {
            closest = Some(candidate.as_str());
            distance = candidate_distance;
            is_unique = true;
        } else if candidate_distance == distance {
            is_unique = false;
        }
    }

    (is_unique && distance <= 2).then_some(closest?)
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    for (left_index, left_character) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right.len() + 1);
        current.push(left_index + 1);
        for (right_index, right_character) in right.iter().copied().enumerate() {
            let replace = previous[right_index] + usize::from(left_character != right_character);
            let insert = current[right_index] + 1;
            let remove = previous[right_index + 1] + 1;
            current.push(replace.min(insert).min(remove));
        }
        previous = current;
    }
    previous[right.len()]
}

impl LocaleWithSchemaInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let schema_label = input.parse::<Ident>()?;
        if schema_label != "schema_path" {
            return Err(syn::Error::new(
                schema_label.span(),
                "expected `schema_path`",
            ));
        }
        input.parse::<Token![:]>()?;
        let schema_path = input.parse()?;
        input.parse::<Token![;]>()?;

        let fallback_label = input.parse::<Ident>()?;
        if fallback_label != "fallback_path" {
            return Err(syn::Error::new(
                fallback_label.span(),
                "expected `fallback_path`",
            ));
        }
        input.parse::<Token![:]>()?;
        let fallback_path = input.parse()?;
        input.parse::<Token![;]>()?;

        let schema_label = input.parse::<Ident>()?;
        if schema_label != "schema" {
            return Err(syn::Error::new(schema_label.span(), "expected `schema`"));
        }
        let schema_content;
        syn::braced!(schema_content in input);
        let schema_messages = parse_schema_messages(&schema_content)?;

        let locale_label = input.parse::<Ident>()?;
        if locale_label != "locale" {
            return Err(syn::Error::new(locale_label.span(), "expected `locale`"));
        }
        let locale_content;
        syn::braced!(locale_content in input);
        let locale_messages = parse_locale_messages(&locale_content)?;

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after locale catalog"));
        }

        Ok(Self {
            schema_path,
            fallback_path,
            schema_messages,
            locale_messages,
        })
    }
}

fn parse_schema_messages(input: ParseStream<'_>) -> Result<Vec<MessageSchema>> {
    let mut messages = Vec::new();
    let mut keys = HashSet::new();
    while !input.is_empty() {
        let is_dynamic = if input.peek(Token![static]) {
            input.parse::<Token![static]>()?;
            false
        } else {
            let kind = input.parse::<Ident>()?;
            if kind != "dynamic" {
                return Err(syn::Error::new(
                    kind.span(),
                    "expected `static` or `dynamic` schema message",
                ));
            }
            true
        };
        let key = input.parse::<Ident>()?;
        if !keys.insert(key.to_string()) {
            return Err(syn::Error::new(
                key.span(),
                format!("message `{key}` has a duplicate schema key"),
            ));
        }
        let arguments = if is_dynamic {
            parse_arguments(input, &key, true)?.0
        } else {
            Vec::new()
        };
        input.parse::<Token![;]>()?;
        messages.push(MessageSchema {
            key,
            is_dynamic,
            arguments,
        });
    }
    Ok(messages)
}

fn parse_locale_messages(input: ParseStream<'_>) -> Result<Vec<Message>> {
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
        let (arguments, has_rest) = parse_arguments(input, &key, false)?;
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
    Ok(messages)
}

fn parse_arguments(
    input: ParseStream<'_>,
    key: &Ident,
    required: bool,
) -> Result<(Vec<MessageArgument>, bool)> {
    if !input.peek(syn::token::Brace) {
        if required {
            return Err(input.error(format!("dynamic schema message `{key}` requires fields")));
        }
        return Ok((Vec::new(), false));
    }

    let content;
    syn::braced!(content in input);
    let mut arguments = Vec::new();
    let mut names = HashSet::new();
    while !content.is_empty() {
        if content.peek(Token![..]) {
            let rest = content.parse::<Token![..]>()?;
            if required {
                return Err(syn::Error::new(
                    rest.spans[0],
                    "`..` is not valid in default schema fields",
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

    use super::{
        CatalogInvocation, LocaleRelayInput, LocaleWithSchemaInput, validate_locale_schema,
    };

    #[test]
    fn authored_locale_relay_preserves_the_raw_message_token_body() {
        let invocation = parse_str(
            r#"
                schema: super::en_us;
                About = "關於";
                Progress { percent, .. } = if enabled { "{percent}" } else { "off" };
            "#,
        )
        .expect("authored locale relay should parse");
        let CatalogInvocation::LocaleRelay(LocaleRelayInput {
            schema_path,
            fallback_path,
            locale_tokens,
        }) = invocation
        else {
            panic!("expected an authored locale relay");
        };

        assert_eq!(schema_path.to_token_stream().to_string(), "super :: en_us");
        assert!(fallback_path.is_none());
        let tokens = locale_tokens.to_string();
        assert!(tokens.contains("About = \"關於\" ;"));
        assert!(tokens.contains("Progress { percent , .. }"));
        assert!(tokens.contains("if enabled { \"{percent}\" } else { \"off\" }"));
    }

    #[test]
    fn authored_locale_relay_preserves_an_explicit_fallback_path_and_raw_body() {
        let relay = parse_str::<LocaleRelayInput>(
            "schema: super::en_us; fallback: super::zh_hant; Greeting = \"hi\";",
        )
        .expect("authored locale relay with an explicit parent should parse");

        assert_eq!(
            relay.schema_path.to_token_stream().to_string(),
            "super :: en_us"
        );
        assert_eq!(
            relay
                .fallback_path
                .expect("explicit fallback path should be retained")
                .to_token_stream()
                .to_string(),
            "super :: zh_hant"
        );
        assert_eq!(relay.locale_tokens.to_string(), "Greeting = \"hi\" ;");
    }

    #[test]
    fn authored_locale_relay_rejects_misordered_repeated_and_late_fallback_headers() {
        assert!(
            parse_str::<LocaleRelayInput>(
                "fallback: super::zh_hant; schema: super::en_us; Greeting = \"hi\";",
            )
            .is_err()
        );
        assert!(parse_str::<LocaleRelayInput>(
            "schema: super::en_us; fallback: super::zh_hant; fallback: super::zh_tw; Greeting = \"hi\";",
        )
        .is_err());
        assert!(
            parse_str::<LocaleRelayInput>(
                "schema: super::en_us; Greeting = \"hi\"; fallback: super::zh_hant;",
            )
            .is_err()
        );

        let old_order_error = match parse_str::<CatalogInvocation>(
            "fallback: super::zh_hant; schema: super::en_us; Greeting = \"hi\";",
        ) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("fallback-first authored locale must fail at the root header"),
        };
        assert!(old_order_error.contains("expected `default` or `schema`"));
    }

    #[test]
    fn locale_callback_requires_and_preserves_a_resolved_fallback_path() {
        let callback = parse_str::<CatalogInvocation>(
            r#"
                @locale_with_schema;
                schema_path: en_us;
                fallback_path: zh_hant;
                schema { static Greeting; }
                locale { Greeting = "你好"; }
            "#,
        )
        .expect("relay callback with a resolved parent should parse");
        let CatalogInvocation::LocaleWithSchema(callback) = callback else {
            panic!("expected relayed locale callback");
        };

        assert_eq!(callback.schema_path.to_token_stream().to_string(), "en_us");
        assert_eq!(
            callback.fallback_path.to_token_stream().to_string(),
            "zh_hant"
        );
    }

    #[test]
    fn schema_matches_follow_locale_order_while_diagnostics_keep_schema_order() {
        let valid = parse_str(
            r#"
            @locale_with_schema;
            schema_path: en_us;
            fallback_path: en_us;
            schema { dynamic First { one, two }; dynamic Second { three, four }; }
            locale { Second { three, four } = ""; First { one, two } = ""; }
        "#,
        )
        .expect("relay invocation should parse");
        let CatalogInvocation::LocaleWithSchema(LocaleWithSchemaInput {
            schema_messages,
            locale_messages,
            ..
        }) = valid
        else {
            panic!("expected relayed catalog")
        };
        let ordered = validate_locale_schema(&schema_messages, &locale_messages)
            .expect("known messages validate");
        assert_eq!(
            ordered
                .iter()
                .map(|message| message.key.to_string())
                .collect::<Vec<_>>(),
            ["Second", "First"]
        );

        let invalid = parse_str(
            r#"
            @locale_with_schema;
            schema_path: en_us;
            fallback_path: en_us;
            schema { dynamic First { one, two }; dynamic Second { three, four }; }
            locale { Second { zzz } = ""; }
        "#,
        )
        .expect("relay invocation should parse");
        let CatalogInvocation::LocaleWithSchema(LocaleWithSchemaInput {
            schema_messages,
            locale_messages,
            ..
        }) = invalid
        else {
            panic!("expected relayed catalog")
        };
        let Err(matches) = validate_locale_schema(&schema_messages, &locale_messages) else {
            panic!("unknown field should fail")
        };
        assert!(
            matches
                .to_string()
                .contains("valid default schema fields: three, four")
        );
    }

    #[test]
    fn unknown_locale_field_lists_authored_schema_fields_in_order() {
        let invocation = match parse_str(
            r#"
                @locale_with_schema;
                schema_path: en_us;
                fallback_path: en_us;
                schema {
                    dynamic Count { first, second, third };
                }
                locale {
                    Count { zzz } = "unused";
                }
            "#,
        ) {
            Ok(invocation) => invocation,
            Err(error) => panic!("relay invocation should parse: {error}"),
        };
        let CatalogInvocation::LocaleWithSchema(LocaleWithSchemaInput {
            schema_messages,
            locale_messages,
            ..
        }) = invocation
        else {
            panic!("expected a relayed locale catalog");
        };

        let Err(error) = validate_locale_schema(&schema_messages, &locale_messages) else {
            panic!("unknown locale field should fail validation");
        };

        assert!(
            error
                .to_string()
                .contains("valid default schema fields: first, second, third")
        );
    }

    #[test]
    fn unknown_locale_field_on_zero_field_schema_names_missing_fields() {
        let invocation = parse_str(
            r#"
                @locale_with_schema;
                schema_path: en_us;
                fallback_path: en_us;
                schema { dynamic Heartbeat {}; }
                locale { Heartbeat { extra } = "unused"; }
            "#,
        )
        .expect("relay invocation should parse");
        let CatalogInvocation::LocaleWithSchema(LocaleWithSchemaInput {
            schema_messages,
            locale_messages,
            ..
        }) = invocation
        else {
            panic!("expected a relayed locale catalog");
        };

        let Err(error) = validate_locale_schema(&schema_messages, &locale_messages) else {
            panic!("unknown locale field must fail validation");
        };

        let message = error.to_string();
        assert!(message.contains("`extra`"));
        assert!(message.contains("`Heartbeat`"));
        assert!(message.contains("default schema message has no fields"));
    }

    #[test]
    fn locale_rest_marker_round_trips_through_the_relay_parser() {
        let invocation = parse_str(
            r#"
                @locale_with_schema;
                schema_path: en_us;
                fallback_path: en_us;
                schema {
                    dynamic Count { count, total };
                }
                locale {
                    Count { count, .. } = "{count}";
                }
            "#,
        )
        .expect("relay invocation should parse");
        let CatalogInvocation::LocaleWithSchema(LocaleWithSchemaInput {
            locale_messages, ..
        }) = invocation
        else {
            panic!("expected a relayed locale catalog");
        };

        assert!(locale_messages[0].has_rest);
        assert_eq!(locale_messages[0].arguments.len(), 1);
    }

    #[test]
    fn edit_distance_counts_unicode_identifier_characters() {
        assert_eq!(super::edit_distance("食物", "食べ物"), 1);
    }
}
