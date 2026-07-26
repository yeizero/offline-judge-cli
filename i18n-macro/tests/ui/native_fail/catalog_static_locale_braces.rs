use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! {
        default;
        About = "About";
    }
}

mod zh_tw {
    use super::catalog;

    catalog! {
        schema: super::en_us;
        About {} = "關於";
    }
}

fn main() {}
