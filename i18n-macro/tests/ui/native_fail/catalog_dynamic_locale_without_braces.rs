use i18n_macro::catalog;

mod en_us {
    use super::catalog;

    catalog! {
        default;
        Heartbeat {} = "alive";
    }
}

mod zh_tw {
    use super::catalog;

    catalog! {
        schema: super::en_us;
        Heartbeat = "存活";
    }
}

fn main() {}
