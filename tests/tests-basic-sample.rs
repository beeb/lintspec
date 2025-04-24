use std::path::PathBuf;

#[cfg(feature = "solar")]
use lintspec::parser::solar::SolarParser;

use lintspec::{
    config::{NoticeDevRules, Req, VariableConfig, WithParamsRules},
    lint::{lint, FileDiagnostics, ValidationOptions},
    parser::slang::SlangParser,
    print_reports,
};
use similar_asserts::assert_eq;

fn generate_output(diags: FileDiagnostics) -> String {
    let mut buf = Vec::new();
    print_reports(&mut buf, PathBuf::new(), diags, true).unwrap();
    String::from_utf8(buf).unwrap()
}

fn multi_lint_handler(
    path: &str,
    options: &ValidationOptions,
    keep_contents: bool,
) -> (FileDiagnostics, FileDiagnostics) {
    let diags_slang = lint(SlangParser::builder().build(), path, options, keep_contents)
        .unwrap()
        .unwrap();

    let diags_solar = lint(SolarParser {}, path, options, keep_contents)
        .unwrap()
        .unwrap();

    (diags_slang, diags_solar)
}

#[test]
fn test_basic() {
    let (diags_slang, diags_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    );

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );

    insta::assert_snapshot!("basic", generate_output(diags_slang));
}

#[test]
fn test_inheritdoc() {
    let (diags_slang, diags_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::default(),
        true,
    );

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_constructor() {
    let (diags_slang, diags_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .constructors(WithParamsRules::required())
            .build(),
        true,
    );

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_struct() {
    let (diags_slang, diags_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .structs(WithParamsRules::required())
            .build(),
        true,
    );

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_enum() {
    let (diags_slang, diags_solar) = multi_lint_handler(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .enums(WithParamsRules::required())
            .build(),
        true,
    );

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_all() {
    let (diags_slang, diags_solar) = multi_lint_handler(
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

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}

#[test]
fn test_all_no_inheritdoc() {
    let (diags_slang, diags_solar) = multi_lint_handler(
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

    assert_eq!(
        generate_output(diags_slang.clone()),
        generate_output(diags_solar)
    );
    insta::assert_snapshot!(generate_output(diags_slang));
}
