use lintspec::lint::ValidationOptions;

mod common;
use common::*;

#[test]
fn test_basic() {
    let (diags_slang, diags_solar) = multi_lint_handler(
        "./test-data/InterfaceSample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    );

    assert!(diags_slang.is_none(), "{diags_slang:?}");
    assert!(diags_solar.is_none(), "{diags_solar:?}");
}
