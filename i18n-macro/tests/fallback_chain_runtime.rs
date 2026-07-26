use i18n_macro::{catalog, define_i18n};

mod en_us {
    use super::catalog;

    catalog! {
        default;
        Greeting = "English";
        Farewell = "Bye";
        ParentDynamic { value } = "English parent {value}";
        DefaultDynamic { value } = "English default {value}";
    }
}

mod zh_hant {
    use super::catalog;

    catalog! {
        schema: super::en_us;
        fallback: super::en_us;
        Greeting = "繁中";
        ParentDynamic { value } = "繁中 parent {value}";
    }
}

mod zh_tw {
    use super::catalog;

    catalog! { schema: super::en_us; fallback: super::zh_hant; }
}

mod zh_hk {
    use super::catalog;

    catalog! { schema: super::en_us; fallback: super::zh_hant; }
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
    locale ZhHant: zh_hant;
    locale ZhTw: zh_tw;
    locale ZhHk: zh_hk;
}

#[test]
fn fallback_chain_renders_parent_messages() {
    assert_eq!(tr_for!(Locale::ZhHant, Greeting), "繁中");
    assert_eq!(tr_for!(Locale::ZhTw, Greeting), "繁中");
    assert_eq!(tr_for!(Locale::ZhTw, Farewell), "Bye");
    assert_eq!(tr_for!(Locale::ZhHk, Farewell), "Bye");
    assert_eq!(
        tr_for!(Locale::ZhTw, ParentDynamic { value: "台灣" }).to_string(),
        "繁中 parent 台灣",
    );
    assert_eq!(
        tr_for!(Locale::ZhHk, ParentDynamic { value: "香港" }).to_string(),
        "繁中 parent 香港",
    );
    assert_eq!(
        tr_for!(Locale::ZhTw, DefaultDynamic { value: "default" }).to_string(),
        "English default default",
    );
}
