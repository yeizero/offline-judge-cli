use proc_macro2::TokenStream;
use quote::quote;
use syn::{Block, Expr, Ident, Lit, Result, Stmt, spanned::Spanned as _};

pub fn lower_message_expression(expression: &Expr, formatter: &Ident) -> Result<TokenStream> {
    lower_expression(expression, formatter)
}

pub fn lower_expression(expression: &Expr, formatter: &Ident) -> Result<TokenStream> {
    match expression {
        Expr::Lit(expression) => {
            let Lit::Str(template) = &expression.lit else {
                return Err(syn::Error::new(
                    expression.span(),
                    "catalog message branches must select string templates",
                ));
            };
            Ok(quote! {
                ::core::fmt::Write::write_fmt(
                    #formatter,
                    ::core::format_args!(#template)
                )
            })
        }
        Expr::Block(expression) => {
            let attributes = &expression.attrs;
            let block = lower_block(&expression.block, formatter)?;
            Ok(quote!(#(#attributes)* #block))
        }
        Expr::If(expression) => {
            let attributes = &expression.attrs;
            let condition = &expression.cond;
            let then_branch = lower_block(&expression.then_branch, formatter)?;
            let Some((_, else_branch)) = &expression.else_branch else {
                return Err(syn::Error::new(
                    expression.span(),
                    "catalog message `if` expressions require an `else` branch",
                ));
            };
            let else_branch = lower_expression(else_branch, formatter)?;
            Ok(quote! {
                #(#attributes)*
                if #condition #then_branch else #else_branch
            })
        }
        Expr::Match(expression) => {
            let attributes = &expression.attrs;
            let matched = &expression.expr;
            let arms = expression
                .arms
                .iter()
                .map(|arm| {
                    let attributes = &arm.attrs;
                    let pattern = &arm.pat;
                    let guard = arm.guard.as_ref().map(|(_, guard)| quote!(if #guard));
                    let body = lower_expression(&arm.body, formatter)?;
                    Ok(quote!(#(#attributes)* #pattern #guard => #body,))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(quote! {
                #(#attributes)*
                match #matched {
                    #(#arms)*
                }
            })
        }
        Expr::Paren(expression) => {
            let attributes = &expression.attrs;
            let inner = lower_expression(&expression.expr, formatter)?;
            Ok(quote!(#(#attributes)* (#inner)))
        }
        Expr::Group(expression) => {
            let attributes = &expression.attrs;
            let inner = lower_expression(&expression.expr, formatter)?;
            Ok(quote!(#(#attributes)* (#inner)))
        }
        _ => Ok(quote!(#expression)),
    }
}

fn lower_block(block: &Block, formatter: &Ident) -> Result<TokenStream> {
    let Some((last, statements)) = block.stmts.split_last() else {
        return Err(syn::Error::new(
            block.span(),
            "catalog message blocks must end in a string template",
        ));
    };
    let Stmt::Expr(tail, None) = last else {
        return Err(syn::Error::new(
            last.span(),
            "catalog message blocks must end in a template expression without a semicolon",
        ));
    };
    let tail = lower_expression(tail, formatter)?;

    Ok(quote!({
        #(#statements)*
        #tail
    }))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::{Expr, Ident, parse_str};

    use super::lower_expression;

    #[test]
    fn lowers_direct_templates_with_repetition_specs_and_brace_escaping() {
        // Catches reparsing Rust format strings with a reduced placeholder grammar.
        assert_lowered(
            r#""{{{value:>8}}} {value}""#,
            &quote! {
                ::core::fmt::Write::write_fmt(
                    __i18n_formatter,
                    ::core::format_args!("{{{value:>8}}} {value}")
                )
            },
        );
    }

    #[test]
    fn lowers_block_tail_without_moving_local_bindings() {
        // Catches dropping native statements or lowering their identifiers out of scope.
        assert_lowered(
            r#"{ let label = "x"; "{label} {label}" }"#,
            &quote! {
                {
                    let label = "x";
                    ::core::fmt::Write::write_fmt(
                        __i18n_formatter,
                        ::core::format_args!("{label} {label}")
                    )
                }
            },
        );
    }

    #[test]
    fn lowers_if_branches_and_preserves_the_native_condition() {
        // Catches evaluating both branches or replacing typed Rust conditions.
        assert_lowered(
            r#"if disabled { "Disabled" } else { "{percent} / 100" }"#,
            &quote! {
                if disabled {
                    ::core::fmt::Write::write_fmt(
                        __i18n_formatter,
                        ::core::format_args!("Disabled")
                    )
                } else {
                    ::core::fmt::Write::write_fmt(
                        __i18n_formatter,
                        ::core::format_args!("{percent} / 100")
                    )
                }
            },
        );
    }

    #[test]
    fn lowers_match_arms_with_local_format_capture() {
        // Catches emitting a match binding outside the arm that defines it.
        assert_lowered(
            r#"match count { 0 => "none", n => "{n} items" }"#,
            &quote! {
                match count {
                    0 => ::core::fmt::Write::write_fmt(
                        __i18n_formatter,
                        ::core::format_args!("none")
                    ),
                    n => ::core::fmt::Write::write_fmt(
                        __i18n_formatter,
                        ::core::format_args!("{n} items")
                    ),
                }
            },
        );
    }

    #[test]
    fn preserves_unresolved_native_control_flow_for_rust_type_checking() {
        // Catches turning the deliberately undecided `?` policy into a macro rejection.
        assert_lowered("some_result?", &quote!(some_result?));
    }

    fn assert_lowered(source: &str, expected: &TokenStream) {
        let expression = parse_str::<Expr>(source).expect("test expression should parse");
        let formatter = Ident::new("__i18n_formatter", proc_macro2::Span::mixed_site());
        let actual =
            lower_expression(&expression, &formatter).expect("supported expression should lower");

        assert_eq!(actual.to_string(), expected.to_string());
    }
}
