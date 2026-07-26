use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! {
        default;
        About = "Evaluator";
    }
}

struct Incomplete;

impl en_us::__i18n_catalog::CatalogImpl for Incomplete {
    type Fallback = Self;
}

fn main() {}
