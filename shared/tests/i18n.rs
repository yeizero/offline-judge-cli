use shared::Locale;

const READY: &str = shared::tr_for!(Locale::ZhTw, I18nReady);

#[test]
fn static_translation_is_available_to_external_callers() {
    assert_eq!(READY, "i18n 已就緒");
    assert_eq!(shared::tr!(I18nReady), "i18n 已就緒");
}

#[test]
fn dynamic_translation_formats_the_configured_locale() {
    assert_eq!(
        format!(
            "{}",
            shared::tr_for!(
                Locale::ZhTw,
                UnsupportedConfiguredLocale { locale: "en-US" }
            )
        ),
        "不支援 config.yaml 中設定的語言 en-US；將使用 zh-TW"
    );
}

#[test]
fn empty_english_catalog_inherits_the_fallback_messages() {
    assert_eq!(shared::tr_for!(Locale::EnUs, I18nReady), "i18n 已就緒");
    assert_eq!(
        format!(
            "{}",
            shared::tr_for!(
                Locale::EnUs,
                UnsupportedConfiguredLocale { locale: "fr-FR" }
            )
        ),
        "不支援 config.yaml 中設定的語言 fr-FR；將使用 zh-TW"
    );
}
