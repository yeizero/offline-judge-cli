use std::{
    fmt::{self, Display, Formatter},
    sync::atomic::{AtomicUsize, Ordering},
};

use i18n_macro::{catalog, define_i18n};

mod en_us {
    use super::catalog;

    catalog! {
        default;

        About = "Default only";
        Text { value } = "value {value}";
    }
}

#[must_use]
pub const fn current_locale() -> Locale {
    Locale::EnUs
}

define_i18n! {
    locale: pub Locale;
    current_locale: current_locale;
    schema: en_us;
    default EnUs: en_us;
}

const ABOUT: &str = tr_for!(Locale::EnUs, About);

const fn assert_static(_: &'static str) {}

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

#[test]
fn default_only_router_has_one_variant_and_static_routes() {
    // Catches generating a synthetic locale or losing static macro routing.
    let only_variant = match Locale::EnUs {
        Locale::EnUs => "only",
    };

    assert_eq!(only_variant, "only");
    assert_static(ABOUT);
    assert_eq!(ABOUT, "Default only");
    assert_eq!(tr_for!(Locale::EnUs, About), "Default only");
    assert_eq!(tr!(About), "Default only");
}

#[test]
fn default_only_router_keeps_dynamic_macros_lazy_display_values() {
    // Catches eager dynamic formatting or default-only macro routing failure.
    let calls = AtomicUsize::new(0);
    let value = CountingDisplay {
        value: "later",
        calls: &calls,
    };
    let current = tr!(Text { value: &value });
    let explicit = tr_for!(Locale::EnUs, Text { value: &value });

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(format!("{current}"), "value later");
    assert_eq!(format!("{explicit}"), "value later");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
