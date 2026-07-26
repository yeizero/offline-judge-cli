use i18n_macro::catalog;

catalog! {
    schema: super::en_us;

    About = "評測器";
    Catalog = "目錄";
    Localized = "在地化";
    ExplicitEmpty = "";
    Text { value } = "文字：{value}";
    CatalogImpl { value } = "目錄實作 {value}";
    ReorderedTyped { second: bool, first: usize } = "typed {second}/{first}";
    ReorderedGeneric { second, first } = "generic {second}/{first}";
}
