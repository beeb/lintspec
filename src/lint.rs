use std::{
    fs, io,
    path::{Path, PathBuf},
};

use derive_more::Display;
use serde::Serialize;
use slang_solidity::{
    cst::{NonterminalKind, TextRange},
    parser::Parser,
};

use crate::{
    definitions::{find_items, Parent, ValidationOptions},
    error::{Error, Result},
    utils::detect_solidity_version,
};

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("Error")]
pub struct FileDiagnostics {
    pub path: PathBuf,
    #[serde(skip_serializing)]
    pub contents: String,
    pub items: Vec<ItemDiagnostics>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemDiagnostics {
    pub parent: Option<Parent>,
    pub item_type: ItemType,
    pub name: String,
    pub span: TextRange,
    pub diags: Vec<Diagnostic>,
}

impl ItemDiagnostics {
    pub fn print_compact(
        &self,
        f: &mut impl io::Write,
        path: impl AsRef<Path>,
        root_dir: impl AsRef<Path>,
    ) -> std::result::Result<(), io::Error> {
        let source_name = match path.as_ref().strip_prefix(root_dir.as_ref()) {
            Ok(relative_path) => relative_path.to_string_lossy(),
            Err(_) => path.as_ref().to_string_lossy(),
        };
        writeln!(
            f,
            "{source_name}:{}:{}",
            self.span.start.line + 1,   // lines start at zero in the span
            self.span.start.column + 1, // columsn start at zero in the span
        )?;
        if let Some(parent) = &self.parent {
            writeln!(f, "{} {}.{}", self.item_type, parent, self.name)?;
        } else {
            writeln!(f, "{} {}", self.item_type, self.name)?;
        }
        for diag in &self.diags {
            writeln!(f, "  {}", diag.message)?;
        }
        writeln!(f)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Display)]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
    Constructor,
    Enum,
    Error,
    Event,
    Function,
    Modifier,
    ParsingError,
    Struct,
    Variable,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub span: TextRange,
    pub message: String,
}

pub fn lint(
    path: impl AsRef<Path>,
    options: &ValidationOptions,
    drop_contents: bool,
) -> Result<Option<FileDiagnostics>> {
    let mut contents = fs::read_to_string(&path).map_err(|err| Error::IOError {
        path: path.as_ref().to_path_buf(),
        err,
    })?;
    let solidity_version = detect_solidity_version(&contents)?;

    let parser = Parser::create(solidity_version).expect("parser should initialize");

    let output = parser.parse(NonterminalKind::SourceUnit, &contents);
    if drop_contents {
        // free memory
        contents = String::new();
    }
    if !output.is_valid() {
        let Some(error) = output.errors().first() else {
            return Err(Error::UnknownError);
        };
        return Err(Error::ParsingError(error.to_string()));
    }

    let cursor = output.create_tree_cursor();
    let mut items: Vec<_> = find_items(cursor)
        .into_iter()
        .filter_map(|item| {
            let mut item_diags = item.validate(options);
            if item_diags.diags.is_empty() {
                None
            } else {
                item_diags.diags.sort_unstable_by_key(|d| d.span.start);
                Some(item_diags)
            }
        })
        .collect();
    if items.is_empty() {
        return Ok(None);
    }
    items.sort_unstable_by_key(|i| i.span.start);
    Ok(Some(FileDiagnostics {
        path: path.as_ref().to_path_buf(),
        contents,
        items,
    }))
}
