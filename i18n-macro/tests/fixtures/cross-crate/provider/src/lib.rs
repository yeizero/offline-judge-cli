use i18n_macro_renamed::define_i18n;

#[path = "../catalogs/en_us.rs"]
pub mod en_us;
#[path = "../catalogs/zh_tw.rs"]
pub mod zh_tw;

pub fn current_locale() -> Locale {
    Locale::EnUs
}

define_i18n! {
    locale: pub Locale;
    current_locale: current_locale;
    schema: en_us;
    default EnUs: en_us;
    locale ZhTw: zh_tw;
}
