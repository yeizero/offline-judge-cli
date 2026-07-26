use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! {
        fallback;
        About = "Evaluator";
    }
}

mod zh_tw {
    use super::catalog;

    catalog! {
        schema: super::en_us;
        Unknown = "unknown";
    }
}

fn main() {}
