use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! {
        default;
        Count { count: usize } = "{count}";
    }
}

mod zh_tw {
    use super::catalog;

    catalog! {
        schema: super::en_us;
        Count { counta } = "Foo";
    }
}

fn main() {}
