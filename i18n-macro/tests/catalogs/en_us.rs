use i18n_macro::catalog;

catalog! {
    fallback;

    About = "Evaluator";
    Catalog = "catalog";
    Localized = "localized";
    MissingInZh = "fallback";
    FooBar = "upper";
    Foobar = "lower";
    ExplicitEmpty = "fallback empty";
    MissingDynamicInZh { count: usize } = "{count} fallback";
    Text { value } = "{value}";
    FormatterArgument { __i18n_formatter } = "{__i18n_formatter}";
    FormatterLocal { value } = {
        let __i18n_formatter = value;
        "{__i18n_formatter}"
    };
    InternalMessage { __i18n_message, value } = "{__i18n_message}/{value}";
    InternalCopy { __i18n_copy, value: usize } = "{__i18n_copy}/{value}";
    CatalogImpl { value } = "catalog impl {value}";
    Progress { disabled: bool, percent } =
        if disabled {
            "Disabled"
        } else {
            "{percent} / 100"
        };
    Count { count: usize } =
        match count {
            0 => "No items",
            1 => "1 item",
            n => "{n} items",
        };
    Shadow { count: usize } = {
        let count = 0;
        if count == 0 { "local" } else { "argument" }
    };
    IfLetShadow { value: Option<usize> } =
        if let Some(value) = value {
            match value {
                7 => "if let",
                _ => "other",
            }
        } else {
            "none"
        };
    WhileLetShadow { value: Option<usize> } = {
        let mut current = value;
        let mut rendered = 0;
        while let Some(value) = current {
            rendered = value;
            current = None;
        }
        if rendered == 7 { "while let" } else { "other" }
    };
    ReorderedTyped { first: usize, second: bool } = "{first}/{second}";
    ReorderedGeneric { first, second } = "{first}/{second}";
    CaseValue { value } = "upper {value}";
    Casevalue { value } = "lower {value}";
    Braces { value } = "{{{value}}}";
}
