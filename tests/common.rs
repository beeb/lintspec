use std::path::PathBuf;

use lintspec::parser::solar::SolarParser;

use lintspec::{
    lint::{lint, FileDiagnostics, ValidationOptions},
    parser::slang::SlangParser,
    print_reports,
};

pub fn generate_output(diags: FileDiagnostics) -> String {
    let mut buf = Vec::new();
    print_reports(&mut buf, PathBuf::new(), diags, true).unwrap();
    String::from_utf8(buf).unwrap()
}

pub fn multi_lint_handler(
    path: &str,
    options: &ValidationOptions,
    keep_contents: bool,
) -> (Option<FileDiagnostics>, Option<FileDiagnostics>) {
    let diags_slang = lint(SlangParser::builder().build(), path, options, keep_contents).unwrap();

    let diags_solar = lint(SolarParser {}, path, options, keep_contents).unwrap();

    (diags_slang, diags_solar)
}
