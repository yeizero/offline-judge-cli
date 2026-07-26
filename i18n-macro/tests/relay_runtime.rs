use std::fmt::{self, Display, Formatter};

// The integration-test package is also named `i18n-macro`, so the resolver
// correctly identifies it as `crate`; locale modules still have no bare macro binding.
pub use i18n_macro::catalog;

mod en_us {
    type A0 = usize;

    i18n_macro::catalog! {
        default;

        About = "About";
        Fallback = "fallback static";
        ExplicitEmpty = "fallback empty";
        Count { count: usize, total: usize } = "{count}/{total}";
        Text { value, suffix } = "{value}{suffix}";
        Progress { disabled: bool, percent: usize } =
            if disabled { "Disabled" } else { "{percent} / 100" };
        Summary { correct: usize, incorrect: usize } =
            "correct {correct}, incorrect {incorrect}";
        Borrowed { text } = "fallback {text}";
        MissingDynamic { value } = "fallback {value}";
        AliasCollision { value, z: A0 } = "{value}/{z}";
    }
}

mod zh_tw {
    i18n_macro::catalog! {
        schema: super::en_us;

        About = "關於";
        Fallback = "locale static";
        ExplicitEmpty = "";
        Count { .. } = "有一些項目";
        Text { value, .. } = "文字：{value}";
        Progress { disabled: bool, .. } = if disabled { "停用" } else { "執行中" };
        Summary { incorrect: usize, correct: usize, .. } = "正確 {correct}，錯誤 {incorrect}";
        Borrowed { text } = "文字：{text}";
    }
}

mod schema_only {
    i18n_macro::catalog! {
        schema: super::en_us;
    }
}

mod explicit_schema_parent {
    i18n_macro::catalog! {
        schema: super::en_us;
        fallback: super::en_us;
    }
}

mod raw_identifier {
    type r#A0 = usize;

    i18n_macro::catalog! {
        default;

        RawAliasCollision { value, z: r#A0 } = "{value}/{z}";
    }
}

mod alias_en_us {
    type FallbackCount = usize;

    i18n_macro::catalog! {
        default;
        AliasCount { count: FallbackCount } = "fallback {count}";
    }
}

mod alias_zh_tw {
    type LocaleCount = usize;

    i18n_macro::catalog! {
        schema: super::alias_en_us;
        AliasCount { count: LocaleCount } = "locale {count}";
    }
}

struct Render<M, C> {
    message: M,
    catalog: std::marker::PhantomData<C>,
}

impl<M, C> Display for Render<M, C>
where
    C: en_us::__i18n_catalog::CatalogImpl,
    M: en_us::__i18n_catalog::DynamicMessage<C>,
{
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        en_us::__i18n_catalog::DynamicMessage::<C>::render(&self.message, formatter)
    }
}

fn render<M, C>(message: M) -> String
where
    C: en_us::__i18n_catalog::CatalogImpl,
    M: en_us::__i18n_catalog::DynamicMessage<C>,
{
    Render {
        message,
        catalog: std::marker::PhantomData::<C>,
    }
    .to_string()
}

struct RawRender<M> {
    message: M,
}

impl<M> Display for RawRender<M>
where
    M: raw_identifier::__i18n_catalog::DynamicMessage<raw_identifier::__i18n_catalog::Catalog>,
{
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        raw_identifier::__i18n_catalog::DynamicMessage::<
            raw_identifier::__i18n_catalog::Catalog,
        >::render(&self.message, formatter)
    }
}

fn raw_render<M>(message: M) -> String
where
    M: raw_identifier::__i18n_catalog::DynamicMessage<raw_identifier::__i18n_catalog::Catalog>,
{
    RawRender { message }.to_string()
}

fn alias_render<M>(message: M) -> String
where
    M: alias_en_us::__i18n_catalog::DynamicMessage<alias_zh_tw::__i18n_catalog::Catalog>,
{
    struct AliasRender<M>(M);

    impl<M> Display for AliasRender<M>
    where
        M: alias_en_us::__i18n_catalog::DynamicMessage<alias_zh_tw::__i18n_catalog::Catalog>,
    {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
            alias_en_us::__i18n_catalog::DynamicMessage::<
                alias_zh_tw::__i18n_catalog::Catalog,
            >::render(&self.0, formatter)
        }
    }

    AliasRender(message).to_string()
}

#[test]
fn locale_relay_uses_fallback_message_shape_for_subsets_and_zero_fields() {
    // Catches locale generation inferring static/dynamic kind and generics from its own fields.
    assert_eq!(
        render::<_, zh_tw::__i18n_catalog::Catalog>(en_us::__i18n_schema::Count {
            count: 3,
            total: 10,
        }),
        "有一些項目",
    );
    assert_eq!(
        render::<_, zh_tw::__i18n_catalog::Catalog>(en_us::__i18n_schema::Text {
            value: "內容",
            suffix: "!",
        }),
        "文字：內容",
    );
}

fn assert_progress_signature(
    _: for<'a> fn(&en_us::__i18n_schema::Progress, &mut Formatter<'a>) -> fmt::Result,
) {
}

#[test]
fn locale_method_has_no_signature_witness_parameter() {
    // Catches retaining the marker-based third CatalogImpl method parameter.
    assert_progress_signature(
        <zh_tw::__i18n_catalog::Catalog as en_us::__i18n_catalog::CatalogImpl>::Progress,
    );
}

#[test]
fn locale_uses_typed_subsets_reordered_fields_and_redundant_rest() {
    // Catches binding all schema fields, matching typed fields by position, or rejecting redundant `..`.
    assert_eq!(
        render::<_, zh_tw::__i18n_catalog::Catalog>(en_us::__i18n_schema::Progress {
            disabled: true,
            percent: 35,
        }),
        "停用",
    );
    assert_eq!(
        render::<_, zh_tw::__i18n_catalog::Catalog>(en_us::__i18n_schema::Progress {
            disabled: false,
            percent: 35,
        }),
        "執行中",
    );
    assert_eq!(
        render::<_, zh_tw::__i18n_catalog::Catalog>(en_us::__i18n_schema::Summary {
            correct: 7,
            incorrect: 2,
        }),
        "正確 7，錯誤 2",
    );
}

#[test]
fn locale_borrows_untyped_values_and_delegates_missing_dynamic_entries() {
    // Catches forcing untyped values to one ownership shape or suppressing fallback delegation.
    let text: &str = "內容";
    let once = &text;
    let twice = &once;

    assert_eq!(
        render::<_, zh_tw::__i18n_catalog::Catalog>(en_us::__i18n_schema::Borrowed { text }),
        "文字：內容",
    );
    assert_eq!(
        render::<_, zh_tw::__i18n_catalog::Catalog>(en_us::__i18n_schema::Borrowed { text: twice }),
        "文字：內容",
    );
    assert_eq!(
        render::<_, zh_tw::__i18n_catalog::Catalog>(en_us::__i18n_schema::MissingDynamic {
            value: "delegated",
        }),
        "fallback delegated",
    );
}

#[test]
fn locale_explicit_empty_static_translation_overrides_fallback() {
    // Catches treating an authored empty static translation as a missing entry.
    assert_eq!(
        <en_us::__i18n_schema::ExplicitEmpty as en_us::__i18n_catalog::StaticMessage<
            zh_tw::__i18n_catalog::Catalog,
        >>::VALUE,
        "",
    );
}

#[test]
fn locale_without_fallback_header_routes_to_the_schema_catalog() {
    assert_eq!(
        <en_us::__i18n_schema::Fallback as en_us::__i18n_catalog::StaticMessage<
            schema_only::__i18n_catalog::Catalog,
        >>::VALUE,
        "fallback static",
    );
    assert_eq!(
        render::<_, schema_only::__i18n_catalog::Catalog>(en_us::__i18n_schema::MissingDynamic {
            value: "schema parent",
        }),
        "fallback schema parent",
    );
    assert_eq!(
        <en_us::__i18n_schema::Fallback as en_us::__i18n_catalog::StaticMessage<
            explicit_schema_parent::__i18n_catalog::Catalog,
        >>::VALUE,
        "fallback static",
    );
}

#[test]
fn authored_a0_type_is_not_captured_by_a_generated_generic() {
    // Catches generic A0 shadowing a fallback module type alias named A0.
    assert_eq!(
        render::<_, en_us::__i18n_catalog::Catalog>(en_us::__i18n_schema::AliasCollision {
            value: "display",
            z: 7,
        }),
        "display/7",
    );
}

#[test]
fn authored_raw_a0_type_is_not_captured_by_a_generated_generic() {
    // Catches treating r#A0 as distinct from an ordinary generated A0 generic.
    assert_eq!(
        raw_render(raw_identifier::__i18n_schema::RawAliasCollision {
            value: "display",
            z: 9,
        }),
        "display/9",
    );
}

#[test]
fn static_fallback_key_routes_like_any_other_static_message() {
    // Catches the CatalogImpl fallback-associated type colliding with a static Fallback key.
    assert_eq!(
        <en_us::__i18n_schema::Fallback as en_us::__i18n_catalog::StaticMessage<
            zh_tw::__i18n_catalog::Catalog,
        >>::VALUE,
        "locale static",
    );
}

#[test]
fn locale_typed_aliases_are_checked_by_rust_not_token_spelling() {
    // Catches token-string comparison rejecting different local aliases of the same Rust type.
    assert_eq!(
        alias_render(alias_en_us::__i18n_schema::AliasCount { count: 12 }),
        "locale 12",
    );
}
