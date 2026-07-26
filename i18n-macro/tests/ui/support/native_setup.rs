use i18n_macro::define_i18n;

#[path = "../../catalogs/en_us.rs"]
mod en_us;
#[path = "../../catalogs/zh_tw.rs"]
mod zh_tw;

fn current_locale() -> Locale {
    Locale::EnUs
}

define_i18n! {
    locale: Locale;
    current_locale: current_locale;
    schema: en_us;
    default EnUs: en_us;
    locale ZhTw: zh_tw;
}
