#![cfg(feature = "solar")]
#![expect(clippy::unwrap_used)]
use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use lintspec_core::{
    lint::{FileDiagnostics, ItemDiagnostics, ValidationOptions, lint},
    parser::{Parse as _, solar::SolarParser},
};
use miette::{LabeledSpan, MietteDiagnostic, NamedSource};

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
        let parser = lintspec_core::parser::slang::SlangParser::default();
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
    diags: Option<FileDiagnostics>,
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

/// Print the reports for a given file, either as pretty or compact text output
///
/// The root path is the current working directory used to compute relative paths if possible. If the file path is
/// not a child of the root path, then the full canonical path of the file is used instead.
/// The writer can be anything that implement [`io::Write`].
pub fn print_reports(
    f: &mut impl io::Write,
    root_path: impl AsRef<Path>,
    file_diags: FileDiagnostics,
    contents: String,
    compact: bool,
) -> Result<(), io::Error> {
    fn inner(
        f: &mut impl io::Write,
        root_path: &Path,
        file_diags: FileDiagnostics,
        contents: String,
        compact: bool,
    ) -> Result<(), io::Error> {
        if compact {
            for item_diags in file_diags.items {
                item_diags.print_compact(f, &file_diags.path, root_path)?;
            }
        } else {
            let source_name = match file_diags.path.strip_prefix(root_path) {
                Ok(relative_path) => relative_path.to_string_lossy(),
                Err(_) => file_diags.path.to_string_lossy(),
            };
            let source = Arc::new(NamedSource::new(source_name, contents));
            for item_diags in file_diags.items {
                print_report(f, Arc::clone(&source), item_diags)?;
            }
        }
        Ok(())
    }
    inner(f, root_path.as_ref(), file_diags, contents, compact)
}

/// Print a single report related to one source item with [`miette`].
///
/// The writer can be anything that implement [`io::Write`].
fn print_report(
    f: &mut impl io::Write,
    source: Arc<NamedSource<String>>,
    item: ItemDiagnostics,
) -> Result<(), io::Error> {
    let msg = if let Some(parent) = &item.parent {
        format!("{} {}.{}", item.item_type, parent, item.name)
    } else {
        format!("{} {}", item.item_type, item.name)
    };
    let labels: Vec<_> = item
        .diags
        .into_iter()
        .map(|d| {
            LabeledSpan::new(
                Some(d.message),
                d.span.start.utf8,
                d.span.end.utf8 - d.span.start.utf8,
            )
        })
        .collect();
    let report: miette::Report = MietteDiagnostic::new(msg).with_labels(labels).into();
    write!(f, "{:?}", report.with_source_code(source))
}
