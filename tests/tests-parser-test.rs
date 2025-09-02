#![cfg(feature = "slang")]
use lintspec::{config::WithParamsRules, lint::ValidationOptions};

mod common;
use common::*;

#[test]
fn test_basic() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/ParserTest.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    ));
}

#[test]
fn test_inheritdoc() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/ParserTest.sol",
        &ValidationOptions::default(),
        true,
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
    ));
}
