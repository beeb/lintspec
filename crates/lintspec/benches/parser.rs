#![allow(clippy::unwrap_used)]
use std::fs::{self, File};

use divan::{Bencher, black_box};
use lintspec::parser::{Parse, ParsedDocument};

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

fn parse_file(mut parser: impl Parse, path: &str) -> ParsedDocument {
    let file = File::open(path).unwrap();
    parser.parse_document(file, Some(path), false).unwrap()
}

#[cfg(feature = "solar")]
#[divan::bench(args = FILES)]
fn parse_solar(bencher: Bencher, path: &str) {
    let parser = lintspec::parser::solar::SolarParser::default();
    bencher.bench_local(move || black_box(parse_file(parser.clone(), path)));
}

#[cfg(feature = "slang")]
#[divan::bench(args = FILES)]
fn parse_slang(bencher: Bencher, path: &str) {
    let parser = lintspec::parser::slang::SlangParser::builder()
        .skip_version_detection(true)
        .build();
    bencher.bench_local(move || {
        black_box(parse_file(parser.clone(), path));
    });
}

#[cfg(feature = "solar")]
#[divan::bench(args = FILES)]
fn compute_indices(bencher: Bencher, path: &str) {
    use std::path::PathBuf;

    use lintspec::parser::solar::complete_text_ranges;

    let mut parser = lintspec::parser::solar::SolarParser::default();
    let source = black_box(fs::read_to_string(path).unwrap());
    let mut document = black_box(
        parser
            .parse_document(fs::File::open(path).unwrap(), None::<PathBuf>, false)
            .unwrap(),
    );
    bencher.bench_local(|| complete_text_ranges(&source, &mut document.definitions));
}
