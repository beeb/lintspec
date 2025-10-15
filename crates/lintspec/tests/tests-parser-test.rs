#![cfg(feature = "slang")]
use lintspec::{
    config::{ContractRules, Req, WithParamsRules},
    lint::ValidationOptions,
};

mod common;
use common::*;

#[test]
fn test_basic() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/ParserTest.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
        true
    ));
}

#[test]
fn test_inheritdoc() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/ParserTest.sol",
        &ValidationOptions::default(),
        true,
        true
    ));
}

#[test]
fn test_constructor() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/ParserTest.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .constructors(WithParamsRules::required())
            .build(),
        true,
        true
    ));
}

#[test]
fn test_struct() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/ParserTest.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .structs(WithParamsRules::required())
            .build(),
        true,
        true
    ));
}

#[test]
fn test_enum() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/ParserTest.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .enums(WithParamsRules::required())
            .build(),
        true,
        true
    ));
}

#[test]
fn test_contract() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/ParserTest.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .contracts(
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
