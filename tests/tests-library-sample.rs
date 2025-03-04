use std::path::PathBuf;

use lintspec::{
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
        "./test-data/LibrarySample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    )
    .unwrap()
    .unwrap();
    insta::assert_snapshot!(generate_output(diags));
}
