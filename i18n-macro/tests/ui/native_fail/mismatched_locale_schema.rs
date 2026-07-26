use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! {
        fallback;
        Count { count: usize } = "{count}";
    }
}

mod zh_tw {
    use super::catalog;

    catalog! {
        schema: super::en_us;
        Count { count: bool } = "{count}";
    }
}

fn main() {}
