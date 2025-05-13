use lintspec::{
    lint::{lint, FileDiagnostics, ValidationOptions},
    parser::slang::SlangParser,
    print_reports,
};
use std::path::PathBuf;

#[cfg(feature = "solar")]
use lintspec::parser::solar::SolarParser;
#[cfg(feature = "solar")]
use similar_asserts::assert_eq;

#[must_use]
pub fn snapshot_content(path: &str, options: &ValidationOptions, keep_contents: bool) -> String {
    let diags_slang = lint(SlangParser::builder().build(), path, options, keep_contents).unwrap();
    let output = generate_output(diags_slang);

    #[cfg(feature = "solar")]
    {
        let diags_solar = lint(SolarParser {}, path, options, keep_contents).unwrap();
        assert_eq!(output, generate_output(diags_solar));
    }

    output
}

fn generate_output(diags: Option<FileDiagnostics>) -> String {
    let Some(diags) = diags else {
        return String::new();
    };
    let mut buf = Vec::new();
    print_reports(&mut buf, PathBuf::new(), diags, true).unwrap();
    String::from_utf8(buf).unwrap()
}
