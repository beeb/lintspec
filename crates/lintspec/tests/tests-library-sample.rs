#![cfg(feature = "slang")]
use lintspec::{
    config::{ContractRules, Req},
    lint::ValidationOptions,
};

mod common;
use common::*;

#[test]
fn test_library() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/LibrarySample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .libraries(
                ContractRules::builder()
                    .title(Req::Required)
                    .author(Req::Required)
                    .notice(Req::Required)
                    .build()
            )
            .build(),
        true,
        true
    ));
}
