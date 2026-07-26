use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! { default; Greeting = "English"; }
}

mod ja_jp {
    use super::catalog;

    catalog! { default; Greeting = "日本語"; }
}

mod zh_tw {
    use super::catalog;

    catalog! { schema: super::en_us; fallback: super::ja_jp; Greeting = "wrong"; }
}

fn main() {}
