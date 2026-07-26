use cross_provider::Locale;

fn main() {
    assert_eq!(cross_provider::tr!(Static), "Provider English");
    assert_eq!(
        cross_provider::tr_for!(Locale::ZhTw, Static),
        "Provider 繁中"
    );

    let value = "x";
    assert_eq!(
        format!("{}", cross_provider::tr!(Dynamic { value })),
        "English x"
    );
    assert_eq!(
        format!(
            "{}",
            cross_provider::tr_for!(Locale::ZhTw, Dynamic { value })
        ),
        "繁中 x"
    );
    assert_eq!(
        format!(
            "{}",
            cross_provider::tr_for!(
                Locale::ZhTw,
                FormatterArgument {
                    __i18n_formatter: "downstream"
                }
            )
        ),
        "繁中 downstream"
    );
    assert_eq!(
        format!(
            "{}",
            cross_provider::tr_for!(
                Locale::ZhTw,
                InternalMessage {
                    __i18n_message: "cross-crate",
                    value: "fallback"
                }
            )
        ),
        "cross-crate/fallback"
    );
}
