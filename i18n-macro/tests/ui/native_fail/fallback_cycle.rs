use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! { default; Greeting = "English"; }
}

mod cycle_a {
    use super::catalog;

    catalog! { schema: super::en_us; fallback: super::cycle_b; Greeting = "A"; }
}

mod cycle_b {
    use super::catalog;

    catalog! { schema: super::en_us; fallback: super::cycle_a; Greeting = "B"; }
}

fn main() {}
