#![cfg(feature = "solar")]
use lintspec_core::{
    config::{ContractRules, NoticeDevRules, Req, VariableConfig, WithParamsRules},
    lint::ValidationOptions,
};

mod common;
use common::*;

#[test]
fn test_basic() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
        true
    ));
}

#[test]
fn test_inheritdoc() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc_override(true)
            .build(),
        true,
        true
    ));
}

#[test]
fn test_constructor() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/BasicSample.sol",
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
        "./test-data/BasicSample.sol",
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
        "./test-data/BasicSample.sol",
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
        "./test-data/BasicSample.sol",
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

#[test]
fn test_all() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc_override(true)
            .constructors(WithParamsRules::required())
            .enums(WithParamsRules::required())
            .modifiers(WithParamsRules::required())
            .structs(WithParamsRules::required())
            .variables(
                VariableConfig::builder()
                    .private(
                        NoticeDevRules::builder()
                            .notice(Req::Required)
                            .dev(Req::Required)
                            .build(),
                    )
                    .internal(
                        NoticeDevRules::builder()
                            .notice(Req::Required)
                            .dev(Req::Required)
                            .build(),
                    )
                    .build(),
            )
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

#[test]
fn test_all_no_inheritdoc() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .inheritdoc_override(false)
            .constructors(WithParamsRules::required())
            .enums(WithParamsRules::required())
            .modifiers(WithParamsRules::required())
            .structs(WithParamsRules::required())
            .variables(
                VariableConfig::builder()
                    .private(
                        NoticeDevRules::builder()
                            .notice(Req::Required)
                            .dev(Req::Required)
                            .build(),
                    )
                    .internal(
                        NoticeDevRules::builder()
                            .notice(Req::Required)
                            .dev(Req::Required)
                            .build(),
                    )
                    .build(),
            )
            .build(),
        true,
        true
    ));
}
