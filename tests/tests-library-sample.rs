use lintspec::lint::ValidationOptions;

mod common;
use common::*;

#[test]
fn test_basic() {
    let (opt_slang, opt_solar) = multi_lint_handler(
        "./test-data/LibrarySample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    );

    let diags_slang = opt_slang.unwrap();
    let diags_solar = opt_solar.unwrap();

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );

    insta::assert_snapshot!(generate_output(diags_slang));
}
