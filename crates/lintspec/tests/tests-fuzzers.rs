#![cfg(feature = "solar")]
use lintspec::lint::ValidationOptions;

mod common;
use common::*;

#[test]
fn test_fuzzers() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/Fuzzers.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
        true
    ));
}
