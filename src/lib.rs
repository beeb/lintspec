use std::{io, path::Path, sync::Arc};

use lint::{FileDiagnostics, ItemDiagnostics};
use miette::{LabeledSpan, MietteDiagnostic, NamedSource};

pub mod comment;
pub mod config;
pub mod definitions;
pub mod error;
pub mod files;
pub mod lint;
pub mod utils;

/// Print the reports for a given file, either as pretty or compact text output.
///
/// The root path is the current working directory used to compute relative paths if possible. If the file path is
/// not a child of the root path, then the full canonical path of the file is used instead.
pub fn print_reports(
    f: &mut impl io::Write,
    root_path: impl AsRef<Path>,
    file_diags: FileDiagnostics,
    compact: bool,
) -> std::result::Result<(), io::Error> {
    if compact {
        for item_diags in file_diags.items {
            item_diags.print_compact(f, &file_diags.path, &root_path)?;
        }
    } else {
        let source_name = match file_diags.path.strip_prefix(root_path.as_ref()) {
            Ok(relative_path) => relative_path.to_string_lossy(),
            Err(_) => file_diags.path.to_string_lossy(),
        };
        let source = Arc::new(NamedSource::new(source_name, file_diags.contents));
        for item_diags in file_diags.items {
            print_report(f, Arc::clone(&source), item_diags)?;
        }
    }
    Ok(())
}

fn print_report(
    f: &mut impl io::Write,
    source: Arc<NamedSource<String>>,
    item: ItemDiagnostics,
) -> std::result::Result<(), io::Error> {
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
