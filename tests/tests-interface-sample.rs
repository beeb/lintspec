use lintspec::{
    lint::{lint, ValidationOptions},
    parser::slang::SlangParser,
};

#[test]
fn test_basic() {
    let diags = lint(
        SlangParser::default(),
        "./test-data/InterfaceSample.sol",
        &ValidationOptions::builder().inheritdoc(false).build(),
        true,
    )
    .unwrap();
    assert!(diags.is_none(), "{diags:?}");
}
