use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn exported_static_and_dynamic_macros_work_from_a_downstream_crate() {
    // Catches recursive dynamic macro calls resolving in the downstream invocation scope.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cross-crate/Cargo.toml");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let target = std::env::temp_dir().join(format!(
        "i18n-macro-cross-crate-{}-{nonce}",
        std::process::id()
    ));
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let result = Command::new(cargo)
        .args(["run", "--offline", "--locked", "--manifest-path"])
        .arg(&fixture)
        .args(["-p", "cross-consumer"])
        .env("CARGO_TARGET_DIR", &target)
        .output();
    let _ = fs::remove_dir_all(&target);
    let output = match result {
        Ok(output) => output,
        Err(error) => panic!("failed to run cross-crate fixture: {error}"),
    };

    assert!(
        output.status.success(),
        "cross-crate fixture failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
