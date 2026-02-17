#![cfg(feature = "solar")]
use lintspec_core::lint::ValidationOptions;

mod common;
use common::*;

#[test]
fn test_unicode() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/UnicodeSample.sol",
        &ValidationOptions::default(),
        true,
        false // pretty output
    ));
}
