use std::{
    fmt::{self, Display, Formatter},
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};

use i18n_macro::define_i18n;

#[path = "catalogs/en_us.rs"]
mod en_us;
#[path = "catalogs/zh_tw.rs"]
mod zh_tw;

static CURRENT_LOCALE: AtomicU8 = AtomicU8::new(0);
static LOCALE_QUERIES: AtomicUsize = AtomicUsize::new(0);

#[allow(clippy::missing_panics_doc)]
pub fn current_locale() -> Locale {
    LOCALE_QUERIES.fetch_add(1, Ordering::SeqCst);
    match CURRENT_LOCALE.load(Ordering::SeqCst) {
        0 => Locale::EnUs,
        1 => Locale::ZhTw,
        value => panic!("invalid test locale {value}"),
    }
}

define_i18n! {
    locale: pub Locale;
    current_locale: current_locale;
    schema: en_us;

    fallback EnUs: en_us;
    locale ZhTw: zh_tw;
}

#[allow(clippy::redundant_static_lifetimes)]
const ABOUT_EN: &'static str = tr_for!(Locale::EnUs, About);

struct CountingDisplay<'a> {
    value: &'a str,
    calls: &'a AtomicUsize,
}

impl Display for CountingDisplay<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        self.calls.fetch_add(1, Ordering::SeqCst);
        formatter.write_str(self.value)
    }
}

fn set_current(locale: Locale) {
    CURRENT_LOCALE.store(
        match locale {
            Locale::EnUs => 0,
            Locale::ZhTw => 1,
        },
        Ordering::SeqCst,
    );
}

fn reset_locale_queries() {
    LOCALE_QUERIES.store(0, Ordering::SeqCst);
}

const fn assert_static(_: &'static str) {}

const fn assert_locale_derives<T: Copy + std::fmt::Debug + Eq + std::hash::Hash>() {}

#[test]
fn generated_locale_and_static_messages_support_const_routing() {
    // Catches retaining a consumer-defined locale or routing statics at runtime.
    assert_locale_derives::<Locale>();
    assert_static(ABOUT_EN);
    assert_eq!(ABOUT_EN, "Evaluator");
    assert_eq!(tr_for!(Locale::ZhTw, About), "評測器");
}

#[test]
fn omitted_static_and_dynamic_messages_delegate_to_the_fallback_catalog() {
    // Catches treating an omitted locale entry as an error or empty translation.
    assert_eq!(tr_for!(Locale::ZhTw, MissingInZh), "fallback");
    assert_eq!(
        format!("{}", tr_for!(Locale::ZhTw, MissingDynamicInZh { count: 2 })),
        "2 fallback"
    );
}

#[test]
fn an_explicit_empty_translation_does_not_delegate() {
    // Catches confusing an authored empty string with an omitted message.
    assert_eq!(tr_for!(Locale::EnUs, ExplicitEmpty), "fallback empty");
    assert_eq!(tr_for!(Locale::ZhTw, ExplicitEmpty), "");
}

#[test]
fn typed_native_if_and_match_expressions_select_templates() {
    // Catches flattening native expressions into a text-only catalog grammar.
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::EnUs,
                Progress {
                    disabled: true,
                    percent: 35
                }
            )
        ),
        "Disabled"
    );
    assert_eq!(
        format!("{}", tr_for!(Locale::EnUs, Count { count: 0 })),
        "No items"
    );
    assert_eq!(
        format!("{}", tr_for!(Locale::EnUs, Count { count: 1 })),
        "1 item"
    );
    assert_eq!(
        format!("{}", tr_for!(Locale::EnUs, Count { count: 3 })),
        "3 items"
    );
}

#[test]
fn rust_resolves_if_let_while_let_and_ordinary_local_shadowing() {
    // Catches rewriting typed argument paths instead of exposing copied locals.
    assert_eq!(
        format!("{}", tr_for!(Locale::EnUs, IfLetShadow { value: Some(7) })),
        "if let"
    );
    assert_eq!(
        format!(
            "{}",
            tr_for!(Locale::EnUs, WhileLetShadow { value: Some(7) })
        ),
        "while let"
    );
    assert_eq!(
        format!("{}", tr_for!(Locale::EnUs, Shadow { count: 99 })),
        "local"
    );
}

#[test]
fn generated_protocol_names_are_valid_message_keys_for_both_callers() {
    // Catches placing message and protocol types in the same generated namespace.
    set_current(Locale::EnUs);
    assert_eq!(tr!(Catalog), "catalog");
    assert_eq!(tr!(Localized), "localized");
    assert_eq!(
        format!("{}", tr!(CatalogImpl { value: "current" })),
        "catalog impl current"
    );

    assert_eq!(tr_for!(Locale::ZhTw, Catalog), "目錄");
    assert_eq!(tr_for!(Locale::ZhTw, Localized), "在地化");
    assert_eq!(
        format!(
            "{}",
            tr_for!(Locale::ZhTw, CatalogImpl { value: "explicit" })
        ),
        "目錄實作 explicit"
    );
}

#[test]
fn formatter_identifier_hygiene_preserves_argument_and_local_captures() {
    // Catches generated formatter references resolving to user-authored bindings.
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::EnUs,
                FormatterArgument {
                    __i18n_formatter: "argument"
                }
            )
        ),
        "argument"
    );
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::EnUs,
                FormatterLocal {
                    value: "block local"
                }
            )
        ),
        "block local"
    );
}

#[test]
fn generated_internal_bindings_do_not_capture_user_argument_names() {
    // Catches generated message and copy-helper references resolving to earlier user bindings.
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::EnUs,
                InternalMessage {
                    __i18n_message: "message",
                    value: "value"
                }
            )
        ),
        "message/value"
    );
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::EnUs,
                InternalCopy {
                    __i18n_copy: "copy",
                    value: 7
                }
            )
        ),
        "copy/7"
    );
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::ZhTw,
                InternalMessage {
                    __i18n_message: "fallback",
                    value: "delegated"
                }
            )
        ),
        "fallback/delegated"
    );
}

#[test]
fn untyped_owned_generics_render_str_and_double_ref_str() {
    // Catches constraining untyped fields to one borrowed string shape.
    set_current(Locale::EnUs);
    let value_as_str: &str = "single";
    let once = &value_as_str;
    let value_as_double_ref = &once;

    assert_eq!(
        format!(
            "{}",
            tr!(Text {
                value: value_as_str
            })
        ),
        "single"
    );
    assert_eq!(
        format!(
            "{}",
            tr!(Text {
                value: value_as_double_ref
            })
        ),
        "single"
    );
}

#[test]
fn generic_dynamic_callers_accept_shorthand_and_reordered_fields() {
    // Catches replacing direct struct construction with ordered key-specific arms.
    set_current(Locale::EnUs);
    let disabled = false;
    let percent = 42;
    let renamed_percent = 64;

    assert_eq!(
        format!("{}", tr!(Progress { disabled, percent })),
        "42 / 100"
    );
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::EnUs,
                Progress {
                    percent: 17,
                    disabled: false
                }
            )
        ),
        "17 / 100"
    );
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::EnUs,
                Progress {
                    disabled: false,
                    percent: renamed_percent
                }
            )
        ),
        "64 / 100"
    );
}

#[test]
fn locale_schema_ignores_declaration_order_without_swapping_field_types() {
    // Catches treating source order as schema or assigning generics by locale order.
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::ZhTw,
                ReorderedTyped {
                    first: 7,
                    second: true
                }
            )
        ),
        "typed true/7"
    );
    assert_eq!(
        format!(
            "{}",
            tr_for!(
                Locale::ZhTw,
                ReorderedGeneric {
                    first: 11usize,
                    second: "text"
                }
            )
        ),
        "generic text/11"
    );
}

#[test]
fn match_local_capture_and_escaped_braces_use_rust_formatting() {
    // Catches losing match bindings or passing escaped braces through verbatim.
    assert_eq!(
        format!("{}", tr_for!(Locale::EnUs, Count { count: 7 })),
        "7 items"
    );
    assert_eq!(
        format!("{}", tr_for!(Locale::EnUs, Braces { value: "captured" })),
        "{captured}"
    );
}

#[test]
fn case_distinct_keys_remain_distinct() {
    // Catches normalizing key spelling into colliding generated identifiers.
    assert_eq!(tr_for!(Locale::EnUs, FooBar), "upper");
    assert_eq!(tr_for!(Locale::EnUs, Foobar), "lower");
    assert_eq!(
        format!("{}", tr_for!(Locale::EnUs, CaseValue { value: 1 })),
        "upper 1"
    );
    assert_eq!(
        format!("{}", tr_for!(Locale::EnUs, Casevalue { value: 2 })),
        "lower 2"
    );
}

#[test]
fn dynamic_rendering_is_lazy_and_uses_ordinary_ownership() {
    // Catches eager formatting or forced borrowing of caller arguments.
    let calls = AtomicUsize::new(0);
    let value = CountingDisplay {
        value: "later",
        calls: &calls,
    };
    let message = tr_for!(Locale::EnUs, Text { value: &value });

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(format!("{message}"), "later");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn current_and_explicit_locale_routing_have_exact_query_semantics() {
    // Catches repeated snapshots or explicit routing consulting runtime state.
    reset_locale_queries();
    set_current(Locale::ZhTw);
    reset_locale_queries();

    let current = tr!(About);
    assert_eq!(current, "評測器");
    assert_eq!(LOCALE_QUERIES.load(Ordering::SeqCst), 1);

    reset_locale_queries();
    assert_eq!(tr_for!(Locale::EnUs, About), "Evaluator");
    assert_eq!(LOCALE_QUERIES.load(Ordering::SeqCst), 0);
}
