include!("../support/native_setup.rs");

fn main() {
    let _ = tr!(Progress {
        disabled: false,
        percent: 3,
        percent: 4,
    });
}
