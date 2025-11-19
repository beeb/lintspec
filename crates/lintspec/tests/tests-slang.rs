#![cfg(all(feature = "solar", feature = "slang"))]
#![allow(clippy::unwrap_used)]
use lintspec::{
    lint::{ValidationOptions, lint},
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
        SolarParser::default(),
        "./test-data/BasicSample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: serde_json::to_string_pretty(&diags_slang).unwrap(), solar: serde_json::to_string_pretty(&diags_solar).unwrap());
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
        SolarParser::default(),
        "./test-data/Fuzzers.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: serde_json::to_string_pretty(&diags_slang).unwrap(), solar: serde_json::to_string_pretty(&diags_solar).unwrap());
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
        SolarParser::default(),
        "./test-data/InterfaceSample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: serde_json::to_string_pretty(&diags_slang).unwrap(), solar: serde_json::to_string_pretty(&diags_solar).unwrap());
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
        SolarParser::default(),
        "./test-data/LibrarySample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: serde_json::to_string_pretty(&diags_slang).unwrap(), solar: serde_json::to_string_pretty(&diags_solar).unwrap());
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
        SolarParser::default(),
        "./test-data/ParserTest.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: serde_json::to_string_pretty(&diags_slang).unwrap(), solar: serde_json::to_string_pretty(&diags_solar).unwrap());
}

#[test]
fn test_unicode() {
    let diags_slang = lint(
        SlangParser::builder().build(),
        "./test-data/UnicodeSample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let diags_solar = lint(
        SolarParser::default(),
        "./test-data/UnicodeSample.sol",
        &ValidationOptions::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slang: serde_json::to_string_pretty(&diags_slang).unwrap(), solar: serde_json::to_string_pretty(&diags_solar).unwrap());
}
