use crate::parity::compare_tools_list_parity;

#[test]
#[ignore = "Accepted parity gap (see tests/fixtures/accepted_parity_failures.txt); run with --include-ignored"]
fn test_tools_list_order_full() {
    let result = compare_tools_list_parity("full");
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
#[ignore = "Accepted parity gap (see tests/fixtures/accepted_parity_failures.txt); run with --include-ignored"]
fn test_tools_list_order_normal() {
    let result = compare_tools_list_parity("normal");
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
#[ignore = "Accepted parity gap (see tests/fixtures/accepted_parity_failures.txt); run with --include-ignored"]
fn test_tools_list_order_compact() {
    let result = compare_tools_list_parity("compact");
    assert!(result.passed, "Parity failed: {:?}", result.error);
}
