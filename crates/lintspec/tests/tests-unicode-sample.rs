#![cfg(feature = "solar")]
use lintspec::lint::ValidationOptions;
use miette::GraphicalTheme;

mod common;
use common::*;

#[test]
fn test_unicode() {
    let _ = miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .graphical_theme(GraphicalTheme::none())
                .build(),
        )
    }));
    insta::assert_snapshot!(snapshot_content(
        "./test-data/UnicodeSample.sol",
        &ValidationOptions::default(),
        true,
        false // pretty output
    ));
}
