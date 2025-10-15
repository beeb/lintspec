#![cfg(feature = "slang")]
use lintspec::{
    config::{ContractRules, Req},
    lint::ValidationOptions,
};

mod common;
use common::*;

#[test]
fn test_interface() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/InterfaceSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .interfaces(
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
