#[test]
fn public_root_compile_contracts() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/fail/*.rs");
}
