use std::path::PathBuf;

use ariadne::{Cache, ColorGenerator, Label, Report, ReportKind, Source};
use itertools::Itertools as _;

use error::{Error, Result};
use lint::{Diagnostic, FileDiagnostics};

pub mod comment;
pub mod config;
pub mod definitions;
pub mod error;
pub mod files;
pub mod lint;
pub mod utils;

pub struct SimpleCache {
    pub source: Source,
}

impl SimpleCache {
    pub fn new(src: String) -> Self {
        Self {
            source: Source::from(src),
        }
    }
}

impl Cache<&PathBuf> for &SimpleCache {
    type Storage = String;

    fn fetch(
        &mut self,
        _: &&PathBuf,
    ) -> std::result::Result<&Source<Self::Storage>, Box<dyn std::fmt::Debug + '_>> {
        Ok(&self.source)
    }

    fn display<'a>(&self, path: &&'a PathBuf) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(path.display()))
    }
}

pub fn print_reports(file_diags: FileDiagnostics) -> Result<()> {
    let cache = SimpleCache::new(file_diags.contents);
    // chunk reports by `item` so that we only display one report for it
    for (_, chunk) in &file_diags.diags.into_iter().chunk_by(|d| d.item_span.start) {
        print_report(&file_diags.path, &cache, chunk)?;
    }

    Ok(())
}

pub fn print_report(
    path: &PathBuf,
    cache: &SimpleCache,
    diags: impl Iterator<Item = Diagnostic>,
) -> Result<()> {
    let mut colors = ColorGenerator::new();
    let mut diags = diags.peekable();
    let first = diags
        .peek()
        .expect("there should be at least one diagnostic");

    Report::build(
        ReportKind::Error,
        (path, first.item_span.start.utf8..first.item_span.end.utf8),
    )
    .with_message(if let Some(parent) = &first.parent {
        format!("{} {}:{}", first.item_type, parent, first.item_name)
    } else {
        format!("{} {}", first.item_type, first.item_name)
    })
    .with_labels(diags.map(|d| {
        Label::new((path, d.span.start.utf8..d.span.end.utf8))
            .with_message(d.message)
            .with_color(colors.next())
    }))
    .finish()
    .eprint(cache)
    .map_err(|err| Error::IOError {
        path: "stdout".into(),
        err,
    })?;

    Ok(())
}
