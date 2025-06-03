#![cfg(feature = "solar")]
use lintspec::{
    lint::{lint, ValidationOptions},
    parser::{slang::SlangParser, solar::SolarParser},
};
use similar_asserts::assert_eq;

#[test]
fn test_basic() {
    let diags_slang = lint(
        SlangParser::builder().build(),
        "./test-data/BasicSample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let diags_solar = lint(
        SolarParser {},
        "./test-data/BasicSample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: format!("{diags_slang:#?}"), solar: format!("{diags_solar:#?}"));
}

#[test]
fn test_fuzzers() {
    let diags_slang = lint(
        SlangParser::builder().build(),
        "./test-data/Fuzzers.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let diags_solar = lint(
        SolarParser {},
        "./test-data/Fuzzers.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: format!("{diags_slang:#?}"), solar: format!("{diags_solar:#?}"));
}

#[test]
fn test_interface() {
    let diags_slang = lint(
        SlangParser::builder().build(),
        "./test-data/InterfaceSample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let diags_solar = lint(
        SolarParser {},
        "./test-data/InterfaceSample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: format!("{diags_slang:#?}"), solar: format!("{diags_solar:#?}"));
}

#[test]
fn test_library() {
    let diags_slang = lint(
        SlangParser::builder().build(),
        "./test-data/LibrarySample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let diags_solar = lint(
        SolarParser {},
        "./test-data/LibrarySample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: format!("{diags_slang:#?}"), solar: format!("{diags_solar:#?}"));
}

#[test]
fn test_parsertest() {
    let diags_slang = lint(
        SlangParser::builder().build(),
        "./test-data/ParserTest.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let diags_solar = lint(
        SolarParser {},
        "./test-data/ParserTest.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: format!("{diags_slang:#?}"), solar: format!("{diags_solar:#?}"));
}
