#[path = "common.rs"]
mod common;
use common::*;

use lintspec::{
    config::{NoticeDevRules, Req, VariableConfig, WithParamsRules},
    lint::ValidationOptions,
};
use similar_asserts::assert_eq;

#[test]
fn test_basic() {
    let (opt_slang, opt_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    );

    let diags_slang = opt_slang.unwrap();
    let diags_solar = opt_solar.unwrap();

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );

    insta::assert_snapshot!("basic", generate_output(diags_slang));
}

#[test]
fn test_inheritdoc() {
    let (opt_slang, opt_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::default(),
        true,
    );

    let diags_slang = opt_slang.unwrap();
    let diags_solar = opt_solar.unwrap();

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_constructor() {
    let (opt_slang, opt_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .constructors(WithParamsRules::required())
            .build(),
        true,
    );

    let diags_slang = opt_slang.unwrap();
    let diags_solar = opt_solar.unwrap();

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_struct() {
    let (opt_slang, opt_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .structs(WithParamsRules::required())
            .build(),
        true,
    );

    let diags_slang = opt_slang.unwrap();
    let diags_solar = opt_solar.unwrap();

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_enum() {
    let (opt_slang, opt_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .enums(WithParamsRules::required())
            .build(),
        true,
    );

    let diags_slang = opt_slang.unwrap();
    let diags_solar = opt_solar.unwrap();

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_all() {
    let (opt_slang, opt_solar) = multi_lint_handler(
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
    );

    let diags_slang = opt_slang.unwrap();
    let diags_solar = opt_solar.unwrap();

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_all_no_inheritdoc() {
    let (opt_slang, opt_solar) = multi_lint_handler(
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
    );

    let diags_slang = opt_slang.unwrap();
    let diags_solar = opt_solar.unwrap();

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}
