use std::fs::File;

use divan::{Bencher, black_box};
use lintspec::{
    lint::{Validate as _, ValidationOptions, lint},
    parser::{Parse as _, ParsedDocument, slang::SlangParser},
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

fn parse_file(path: &str) -> ParsedDocument {
    let file = File::open(path).unwrap();
    SlangParser::builder()
        .skip_version_detection(true)
        .build()
        .parse_document(file, Some(path), false)
        .unwrap()
}

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

#[divan::bench(args = FILES)]
fn lint_e2e(bencher: Bencher, path: &str) {
    let parser = SlangParser::builder().skip_version_detection(true).build();
    let options = ValidationOptions::default();
    bencher.bench_local(move || {
        black_box(lint(parser.clone(), path, &options, false).ok());
    });
}
