use i18n_macro::{catalog, define_i18n};

mod en_us {
    use super::catalog;

    catalog! {
        default;
        Heartbeat {} = "alive";
    }
}

fn current_locale() -> Locale {
    Locale::EnUs
}

define_i18n! {
    locale: Locale;
    current_locale: current_locale;
    schema: en_us;
    default EnUs: en_us;
}

fn main() {
    let _ = tr!(Heartbeat);
}
