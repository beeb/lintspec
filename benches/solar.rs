use std::time::Duration;

use divan::{black_box, Bencher};
use lintspec::{
    lint::{lint, ValidationOptions},
    parser::{slang::SlangParser, solar::SolarParser},
};

const FILES: &[&str] = &[
    "test-data/BasicSample.sol",
    "test-data/ParserTest.sol",
    "test-data/InterfaceSample.sol",
    "test-data/LibrarySample.sol",
];

fn main() {
    divan::main();
}

#[divan::bench(args = FILES, min_time = Duration::from_secs(1))]
fn parse_slang(bencher: Bencher, path: &str) {
    let options = ValidationOptions::default();
    let parser = SlangParser::builder().skip_version_detection(true).build();
    bencher.bench_local(move || black_box(lint(parser.clone(), path, &options, false).unwrap()))
}

#[divan::bench(args = FILES, min_time = Duration::from_secs(1))]
fn parse_solar(bencher: Bencher, path: &str) {
    let options = ValidationOptions::default();
    let parser = SolarParser {};
    bencher.bench_local(move || black_box(lint(parser.clone(), path, &options, false).unwrap()))
}
