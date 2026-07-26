use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! {
        fallback;
        About = "Evaluator";
    }
}

struct Incomplete;

impl en_us::__i18n_catalog::CatalogImpl for Incomplete {
    type Fallback = Self;
    const __I18N_GENERATED_CATALOG: () = ();
}

fn main() {}
