use std::path::PathBuf;

use lintspec::{
    config::{FunctionConfig, FunctionRules, NoticeDevRules, Req, VariableConfig, WithParamsRules},
    lint::{lint, FileDiagnostics, ValidationOptions},
    parser::slang::SlangParser,
    print_reports,
};

fn generate_output(diags: FileDiagnostics) -> String {
    let mut buf = Vec::new();
    print_reports(&mut buf, PathBuf::new(), diags, true).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn test_basic() {
    let diags = lint::<SlangParser>(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}

#[test]
fn test_inheritdoc() {
    let diags = lint::<SlangParser>(
        "./test-data/BasicSample.sol",
        &ValidationOptions::default(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}

#[test]
fn test_constructor() {
    let diags = lint::<SlangParser>(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .constructors(WithParamsRules::required())
            .build(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}

#[test]
fn test_struct() {
    let diags = lint::<SlangParser>(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .structs(WithParamsRules::required())
            .build(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}

#[test]
fn test_enum() {
    let diags = lint::<SlangParser>(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .enums(WithParamsRules::required())
            .build(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}

#[test]
fn test_all() {
    let diags = lint::<SlangParser>(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .constructors(WithParamsRules::required())
            .enums(WithParamsRules::required())
            .functions(
                FunctionConfig::builder()
                    .private(FunctionRules::required())
                    .internal(FunctionRules::required())
                    .build(),
            )
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
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}

#[test]
fn test_all_no_inheritdoc() {
    let diags = lint::<SlangParser>(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .constructors(WithParamsRules::required())
            .enums(WithParamsRules::required())
            .functions(
                FunctionConfig::builder()
                    .private(FunctionRules::required())
                    .internal(FunctionRules::required())
                    .build(),
            )
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
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}
