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
        "./test-data/LibrarySample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .constructor(false)
            .struct_params(false)
            .enum_params(false)
            .build(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}
