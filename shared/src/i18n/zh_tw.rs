use i18n_macro::catalog;

catalog! {
    fallback;

    I18nReady = "i18n 已就緒";

    UnsupportedConfiguredLocale { locale } =
        "不支援 config.yaml 中設定的語言 {locale}；將使用 zh-TW";
}
