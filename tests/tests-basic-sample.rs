use std::path::PathBuf;

use lintspec::{
    definitions::ValidationOptions,
    lint::{lint, FileDiagnostics},
    print_reports,
};

fn generate_output(diags: FileDiagnostics) -> String {
    let mut buf = Vec::new();
    print_reports(&mut buf, PathBuf::new(), diags, true).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn test_basic() {
    let diags = lint(
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
    let diags = lint(
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
    let diags = lint(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .constructor(true)
            .build(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}

#[test]
fn test_struct() {
    let diags = lint(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .struct_params(true)
            .build(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}

#[test]
fn test_enum() {
    let diags = lint(
        "./test-data/BasicSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .enum_params(true)
            .build(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}
