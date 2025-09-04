#![cfg(feature = "slang")]
use std::path::PathBuf;

use lintspec::{
    lint::{ValidationOptions, lint},
    parser::slang::SlangParser,
    print_reports,
};

#[allow(unused_variables)]
#[must_use]
pub fn snapshot_content(path: &str, options: &ValidationOptions, keep_contents: bool) -> String {
    let diags_slang = lint(SlangParser::builder().build(), path, options, keep_contents).unwrap();
    let output = generate_output(diags_slang);
    #[cfg(feature = "solar")]
    {
        let diags_solar = lint(
            lintspec::parser::solar::SolarParser::default(),
            path,
            options,
            keep_contents,
        )
        .unwrap();
        similar_asserts::assert_eq!(output, generate_output(diags_solar));
    }
    output
}

fn generate_output(diags: Option<lintspec::lint::FileDiagnostics>) -> String {
    let Some(diags) = diags else {
        return String::new();
    };
    let mut buf = Vec::new();
    print_reports(&mut buf, PathBuf::new(), diags, true).unwrap();
    String::from_utf8(buf).unwrap()
}
