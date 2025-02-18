use std::{
    fs,
    path::{Path, PathBuf},
};

use slang_solidity::{
    cst::{NonterminalKind, TextRange},
    parser::Parser,
};

use crate::{
    definitions::find_items,
    error::{Error, Result},
    utils::detect_solidity_version,
};

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub span: TextRange,
    pub message: String,
}

pub fn lint(path: impl AsRef<Path>) -> Result<Vec<Diagnostic>> {
    let contents = fs::read_to_string(&path)?;
    let solidity_version = detect_solidity_version(&contents)?;

    let parser = Parser::create(solidity_version).expect("parser should initialize");

    let output = parser.parse(NonterminalKind::SourceUnit, &contents);
    if !output.is_valid() {
        let Some(error) = output.errors().first() else {
            return Err(Error::UnknownError);
        };
        return Err(Error::ParsingError(error.to_string()));
    }

    let cursor = output.create_tree_cursor();
    let _ = find_items(cursor);

    Ok(Vec::new())
}
