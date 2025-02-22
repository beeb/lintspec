use lintspec::{definitions::ValidationOptions, lint::lint};

#[test]
fn test_basic() {
    let diags = lint(
        "./test-data/InterfaceSample.sol",
        &ValidationOptions::builder()
            .inheritdoc(false)
            .constructor(false)
            .struct_params(false)
            .enum_params(false)
            .build(),
        true,
    )
    .unwrap();
    assert!(diags.is_none(), "{diags:?}");
}
