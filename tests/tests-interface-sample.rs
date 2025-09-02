#![cfg(feature = "slang")]
use lintspec::lint::ValidationOptions;

mod common;
use common::*;

#[test]
fn test_interface() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/InterfaceSample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    ));
}
