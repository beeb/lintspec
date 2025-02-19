use std::{
    fs,
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
    pub diags: Vec<Diagnostic>,
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
    pub parent: Option<Parent>,
    pub item_type: ItemType,
    pub item_name: String,
    pub item_span: TextRange,
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
    let mut diags = Vec::new();
    for mut item in find_items(cursor) {
        diags.append(&mut item.validate(options));
    }
    if diags.is_empty() {
        return Ok(None);
    }
    diags.sort_unstable_by_key(|d| d.span.start);
    Ok(Some(FileDiagnostics {
        path: path.as_ref().to_path_buf(),
        contents,
        diags,
    }))
}
