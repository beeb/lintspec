use std::path::Path;

use itertools::Itertools as _;

use lint::{Diagnostic, FileDiagnostics};
use miette::{LabeledSpan, MietteDiagnostic, NamedSource};

pub mod comment;
pub mod config;
pub mod definitions;
pub mod error;
pub mod files;
pub mod lint;
pub mod utils;

pub fn print_reports(root_path: impl AsRef<Path>, file_diags: FileDiagnostics) {
    let source_name = match file_diags.path.strip_prefix(root_path.as_ref()) {
        Ok(relative_path) => relative_path.to_string_lossy(),
        Err(_) => file_diags
            .path
            .file_name()
            .expect("path should have a filename")
            .to_string_lossy(),
    };
    let source = NamedSource::new(source_name, file_diags.contents);
    for (_, chunk) in &file_diags.diags.into_iter().chunk_by(|d| d.item_span.start) {
        print_report(source.clone(), chunk);
    }
}

fn print_report(source: NamedSource<String>, diags: impl Iterator<Item = Diagnostic>) {
    let mut diags = diags.peekable();
    let first = diags
        .peek()
        .expect("there should be at least one diagnostic");
    let msg = if let Some(parent) = &first.parent {
        format!("{} {}:{}", first.item_type, parent, first.item_name)
    } else {
        format!("{} {}", first.item_type, first.item_name)
    };
    let labels: Vec<_> = diags
        .map(|d| {
            LabeledSpan::new(
                Some(d.message),
                d.span.start.utf8,
                d.span.end.utf8 - d.span.start.utf8,
            )
        })
        .collect();
    let report: miette::Report = MietteDiagnostic::new(msg).with_labels(labels).into();
    eprintln!("{:?}", report.with_source_code(source));
}
