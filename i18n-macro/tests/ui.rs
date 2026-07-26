#[test]
fn invalid_catalogs_and_calls_fail_at_compile_time() {
    // Catches catalog and caller validation degrading into successful compilation.
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/native_fail/*.rs");
}
