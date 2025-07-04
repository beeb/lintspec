use std::fs::File;

use divan::{Bencher, black_box};
use lintspec::parser::{Parse, ParsedDocument, slang::SlangParser};

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

fn parse_file(mut parser: impl Parse, path: &str) -> ParsedDocument {
    let file = File::open(path).unwrap();
    parser.parse_document(file, Some(path), false).unwrap()
}

#[divan::bench(args = FILES)]
fn parse_slang(bencher: Bencher, path: &str) {
    let parser = SlangParser::builder().skip_version_detection(true).build();
    bencher.bench_local(move || {
        black_box(parse_file(parser.clone(), path));
    });
}

#[cfg(feature = "solar")]
#[divan::bench(args = FILES)]
fn parse_solar(bencher: Bencher, path: &str) {
    let parser = lintspec::parser::solar::SolarParser {};
    bencher.bench_local(move || black_box(parse_file(parser.clone(), path)));
}
