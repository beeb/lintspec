#![expect(clippy::unwrap_used)]
use std::fs::File;

use divan::{Bencher, black_box};
use lintspec_core::{
    lint::{Validate as _, ValidationOptions, lint},
    parser::{Parse as _, ParsedDocument},
};

const FILES: &[&str] = &[
    "test-data/BasicSample.sol",
    "test-data/ParserTest.sol",
    // these are too quick and give a lot of noise on codspeed
    // "test-data/InterfaceSample.sol",
    // "test-data/LibrarySample.sol",
    "test-data/Fuzzers.sol",
];

fn main() {
    divan::main();
}

#[cfg(feature = "solar")]
fn parse_file_solar(path: &str) -> ParsedDocument {
    let file = File::open(path).unwrap();
    lintspec_core::parser::solar::SolarParser::default()
        .parse_document(file, Some(path), false)
        .unwrap()
}

#[cfg(feature = "solar")]
#[divan::bench(args = FILES)]
fn lint_only(bencher: Bencher, path: &str) {
    let doc = parse_file_solar(path);
    let options = ValidationOptions::default();
    bencher.bench_local(move || {
        black_box(
            doc.definitions
                .iter()
                .map(|item| item.validate(&options))
                .collect::<Vec<_>>(),
        );
    });
}

#[cfg(feature = "solar")]
#[divan::bench(args = FILES)]
fn lint_e2e_solar(bencher: Bencher, path: &str) {
    let parser = lintspec_core::parser::solar::SolarParser::default();
    let options = ValidationOptions::default();
    bencher.bench_local(move || {
        black_box(lint(parser.clone(), path, &options, false).ok());
    });
}

#[cfg(feature = "slang")]
#[divan::bench(args = FILES)]
fn lint_e2e_slang(bencher: Bencher, path: &str) {
    let parser = lintspec_core::parser::slang::SlangParser::builder()
        .skip_version_detection(true)
        .build();
    let options = ValidationOptions::default();
    bencher.bench_local(move || {
        black_box(lint(parser.clone(), path, &options, false).ok());
    });
}
