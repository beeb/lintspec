#![cfg(feature = "slang")]
use std::path::PathBuf;

use lintspec::{
    lint::{ValidationOptions, lint},
    parser::{Parse as _, slang::SlangParser},
    print_reports,
};

#[allow(unused_variables)]
#[must_use]
pub fn snapshot_content(path: &str, options: &ValidationOptions, keep_contents: bool) -> String {
    let parser = SlangParser::default();
    let diags_slang = lint(parser.clone(), path, options, keep_contents).unwrap();
    let mut sources = parser.get_sources().unwrap();
    let contents = diags_slang
        .as_ref()
        .and_then(|f| sources.remove(&f.document_id));
    let output = generate_output(diags_slang, contents);
    #[cfg(feature = "solar")]
    {
        let parser = lintspec::parser::solar::SolarParser::default();
        let diags_solar = lint(parser.clone(), path, options, keep_contents).unwrap();
        let mut sources = parser.get_sources().unwrap();
        let contents = diags_solar
            .as_ref()
            .and_then(|f| sources.remove(&f.document_id));
        similar_asserts::assert_eq!(output, generate_output(diags_solar, contents));
    }
    output
}

fn generate_output(
    diags: Option<lintspec::lint::FileDiagnostics>,
    contents: Option<String>,
) -> String {
    let Some(diags) = diags else {
        return String::new();
    };
    let mut buf = Vec::new();
    let Some(contents) = contents else {
        return String::new();
    };
    print_reports(&mut buf, PathBuf::new(), diags, contents, true).unwrap();
    String::from_utf8(buf).unwrap()
}
