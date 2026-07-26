use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! {
        default;
        Count { count: usize, total: usize } = "{count}/{total}";
    }
}

mod zh_tw {
    use super::catalog;

    catalog! {
        schema: super::en_us;
        Count { count: usize } = "{count}";
    }
}

fn main() {}
