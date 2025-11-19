#![cfg(feature = "solar")]
#![allow(clippy::unwrap_used)]
use std::path::PathBuf;

use lintspec::{
    cli::print_reports,
    lint::{ValidationOptions, lint},
    parser::{Parse as _, solar::SolarParser},
};

#[allow(unused_variables)]
#[must_use]
pub fn snapshot_content(
    path: &str,
    options: &ValidationOptions,
    keep_contents: bool,
    compact: bool,
) -> String {
    let parser = SolarParser::default();
    let diags_solar = lint(parser.clone(), path, options, keep_contents).unwrap();
    let mut sources = parser.get_sources().unwrap();
    let contents = diags_solar
        .as_ref()
        .and_then(|f| sources.remove(&f.document_id));
    let output = generate_output(diags_solar, contents, compact);
    #[cfg(feature = "slang")]
    {
        let parser = lintspec::parser::slang::SlangParser::default();
        let diags_slang = lint(parser.clone(), path, options, keep_contents).unwrap();
        let mut sources = parser.get_sources().unwrap();
        let contents = diags_slang
            .as_ref()
            .and_then(|f| sources.remove(&f.document_id));
        similar_asserts::assert_eq!(output, generate_output(diags_slang, contents, compact));
    }
    output
}

fn generate_output(
    diags: Option<lintspec::lint::FileDiagnostics>,
    contents: Option<String>,
    compact: bool,
) -> String {
    let Some(diags) = diags else {
        return String::new();
    };
    let mut buf = Vec::new();
    let Some(contents) = contents else {
        return String::new();
    };
    print_reports(&mut buf, PathBuf::new(), diags, contents, compact).unwrap();
    String::from_utf8(buf).unwrap()
}
