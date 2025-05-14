use lintspec::{
    config::{NoticeDevRules, Req, VariableConfig, WithParamsRules},
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
    ));
}

#[test]
fn test_inheritdoc() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/BasicSample.sol",
        &ValidationOptions::default(),
        true,
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
    ));
}

#[test]
fn test_all() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
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
    ));
}

#[test]
fn test_all_no_inheritdoc() {
    insta::assert_snapshot!(snapshot_content(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
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
    ));
}
