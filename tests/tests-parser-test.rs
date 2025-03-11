use std::path::PathBuf;

use lintspec::{
    config::WithParamsRules,
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
        "./test-data/ParserTest.sol",
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
        "./test-data/ParserTest.sol",
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
        "./test-data/ParserTest.sol",
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
        "./test-data/ParserTest.sol",
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
        "./test-data/ParserTest.sol",
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
