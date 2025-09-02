use std::fs::File;

use divan::{Bencher, black_box};
use lintspec::{
    lint::{Validate as _, ValidationOptions, lint},
    parser::{Parse as _, ParsedDocument},
};

const FILES: &[&str] = &[
    "test-data/BasicSample.sol",
    "test-data/ParserTest.sol",
    "test-data/InterfaceSample.sol",
    "test-data/LibrarySample.sol",
    "test-data/Fuzzers.sol",
];

fn main() {
    divan::main();
}

#[cfg(feature = "slang")]
fn parse_file(path: &str) -> ParsedDocument {
    let file = File::open(path).unwrap();
    lintspec::parser::slang::SlangParser::builder()
        .skip_version_detection(true)
        .build()
        .parse_document(file, Some(path), false)
        .unwrap()
}

#[cfg(feature = "slang")]
#[divan::bench(args = FILES)]
fn lint_only(bencher: Bencher, path: &str) {
    let doc = parse_file(path);
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

#[cfg(feature = "slang")]
#[divan::bench(args = FILES)]
fn lint_e2e(bencher: Bencher, path: &str) {
    let parser = lintspec::parser::slang::SlangParser::builder()
        .skip_version_detection(true)
        .build();
    let options = ValidationOptions::default();
    bencher.bench_local(move || {
        black_box(lint(parser.clone(), path, &options, false).ok());
    });
}

#[cfg(feature = "solar")]
#[divan::bench(args = FILES)]
fn lint_e2e_solar(bencher: Bencher, path: &str) {
    let parser = lintspec::parser::solar::SolarParser {};
    let options = ValidationOptions::default();
    bencher.bench_local(move || {
        black_box(lint(parser.clone(), path, &options, false).ok());
    });
}
