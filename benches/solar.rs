use std::{fs::File, time::Duration};

use divan::{black_box, Bencher};
use lintspec::parser::{slang::SlangParser, solar::SolarParser, Parse, ParsedDocument};

const FILES: &[&str] = &[
    "test-data/BasicSample.sol",
    "test-data/ParserTest.sol",
    "test-data/InterfaceSample.sol",
    "test-data/LibrarySample.sol",
];

fn main() {
    divan::main();
}

fn parse_file(mut parser: impl Parse, path: &str) -> ParsedDocument {
    let file = File::open(path).unwrap();
    parser.parse_document(file, Some(path), false).unwrap()
}

#[divan::bench(args = FILES, min_time = Duration::from_secs(1))]
fn parse_slang(bencher: Bencher, path: &str) {
    let parser = SlangParser::builder().skip_version_detection(true).build();
    bencher.bench_local(move || {
        black_box(parse_file(parser.clone(), path));
    });
}

#[divan::bench(args = FILES, min_time = Duration::from_secs(1))]
fn parse_solar(bencher: Bencher, path: &str) {
    let parser = SolarParser {};
    bencher.bench_local(move || black_box(parse_file(parser.clone(), path)));
}
