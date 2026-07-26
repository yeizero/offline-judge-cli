use i18n_macro::catalog;

catalog! {
    fallback;

    Static = "Provider English";
    Dynamic { value } = "English {value}";
    FormatterArgument { __i18n_formatter } = "{__i18n_formatter}";
    InternalMessage { __i18n_message, value } = "{__i18n_message}/{value}";
}
