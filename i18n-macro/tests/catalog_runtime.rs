use std::fmt::{self, Display, Formatter};

use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! {
        fallback;

        About = "Evaluator";
        MissingStatic = "fallback";
        ExplicitEmpty = "fallback empty";
        MissingDynamic { count: usize } = "{count} fallback";
        Text { value } = "{value}";
        Progress { disabled: bool, percent: usize } =
            if disabled {
                "Disabled"
            } else {
                "{percent} / 100"
            };
        Count { count: usize } =
            match count {
                0 => "No items",
                n => "{n} items",
            };
    }
}

mod zh_tw {
    use super::catalog;

    catalog! {
        schema: super::en_us;

        About = "評測器";
        ExplicitEmpty = "";
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

#[test]
fn fallback_catalog_generates_static_and_typed_dynamic_protocols() {
    // Catches emitting runtime static lookup or untyped native conditions.
    #[allow(clippy::redundant_static_lifetimes)]
    const ABOUT: &'static str =
        <en_us::__i18n_schema::About as en_us::__i18n_catalog::StaticMessage<
            en_us::__i18n_catalog::Catalog,
        >>::VALUE;

    assert_eq!(ABOUT, "Evaluator");
    assert_eq!(
        render::<_, en_us::__i18n_catalog::Catalog>(en_us::__i18n_schema::Progress {
            disabled: true,
            percent: 35,
        }),
        "Disabled"
    );
    assert_eq!(
        render::<_, en_us::__i18n_catalog::Catalog>(en_us::__i18n_schema::Progress {
            disabled: false,
            percent: 35,
        }),
        "35 / 100"
    );
}

#[test]
fn fallback_catalog_generates_owned_generic_values_and_match_capture() {
    // Catches borrowing caller fields or lowering match bindings out of scope.
    let value: &str = "owned generic";

    assert_eq!(
        render::<_, en_us::__i18n_catalog::Catalog>(en_us::__i18n_schema::Text { value }),
        "owned generic"
    );
    assert_eq!(
        render::<_, en_us::__i18n_catalog::Catalog>(en_us::__i18n_schema::Count { count: 0 }),
        "No items"
    );
    assert_eq!(
        render::<_, en_us::__i18n_catalog::Catalog>(en_us::__i18n_schema::Count { count: 4 }),
        "4 items"
    );
}

#[test]
fn partial_catalog_omissions_delegate_but_explicit_empty_overrides() {
    // Catches requiring complete locale catalogs or treating empty as omission.
    assert_eq!(
        <en_us::__i18n_schema::MissingStatic as en_us::__i18n_catalog::StaticMessage<
            zh_tw::__i18n_catalog::Catalog,
        >>::VALUE,
        "fallback"
    );
    assert_eq!(
        <en_us::__i18n_schema::ExplicitEmpty as en_us::__i18n_catalog::StaticMessage<
            zh_tw::__i18n_catalog::Catalog,
        >>::VALUE,
        ""
    );
    assert_eq!(
        render::<_, zh_tw::__i18n_catalog::Catalog>(en_us::__i18n_schema::MissingDynamic {
            count: 2,
        }),
        "2 fallback"
    );
}
