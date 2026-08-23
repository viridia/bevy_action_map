#![allow(missing_docs)]

#[test]
fn derives_compile() {
    let test_cases = trybuild::TestCases::new();
    test_cases.pass("tests/ui/pass/action.rs");
    test_cases.pass("tests/ui/pass/context.rs");
    test_cases.compile_fail("tests/ui/fail/missing_attrs.rs");
    test_cases.compile_fail("tests/ui/fail/missing_action_path.rs");
    test_cases.compile_fail("tests/ui/fail/missing_context_path.rs");
    test_cases.compile_fail("tests/ui/fail/intent_output_mismatch.rs");
}
