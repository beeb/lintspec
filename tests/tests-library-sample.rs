use lintspec::lint::ValidationOptions;

mod common;
use common::*;

#[test]
fn test_library() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/LibrarySample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    ));
}
