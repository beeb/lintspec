use std::{path::Path, sync::Arc};

use lint::{FileDiagnostics, ItemDiagnostics};
use miette::{LabeledSpan, MietteDiagnostic, NamedSource};

pub mod comment;
pub mod config;
pub mod definitions;
pub mod error;
pub mod files;
pub mod lint;
pub mod utils;

pub fn print_reports(root_path: impl AsRef<Path>, file_diags: FileDiagnostics, compact: bool) {
    if compact {
        for item_diags in file_diags.items {
            item_diags.print_compact(&file_diags.path, &root_path);
        }
    } else {
        let source_name = match file_diags.path.strip_prefix(root_path.as_ref()) {
            Ok(relative_path) => relative_path.to_string_lossy(),
            Err(_) => file_diags.path.to_string_lossy(),
        };
        let source = Arc::new(NamedSource::new(source_name, file_diags.contents));
        for item_diags in file_diags.items {
            print_report(Arc::clone(&source), item_diags);
        }
    }
}

fn print_report(source: Arc<NamedSource<String>>, item: ItemDiagnostics) {
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
    eprintln!("{:?}", report.with_source_code(source));
}
