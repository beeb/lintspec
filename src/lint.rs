use std::{
    fs,
    path::{Path, PathBuf},
};

use derive_more::From;
use slang_solidity::{
    cst::{Cursor, NonterminalKind, TextRange},
    parser::Parser,
};

use crate::{
    definitions::{function::FunctionDefinition, structure::StructDefinition, Validate as _},
    error::{Error, Result},
    utils::detect_solidity_version,
};

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub span: TextRange,
    pub message: String,
}

#[derive(Debug, From)]
pub enum Definition {
    Function(FunctionDefinition),
    Struct(StructDefinition),
    NatspecParsingError(Error),
}

impl Definition {
    fn validate(&self, file_path: impl AsRef<Path>) -> Vec<Diagnostic> {
        let path = file_path.as_ref().to_path_buf();
        let mut res = Vec::new();
        match self {
            Definition::NatspecParsingError(error) => {
                let (span, message) = match error {
                    Error::NatspecParsingError { span, message } => (span.clone(), message.clone()),
                    _ => (TextRange::default(), error.to_string()),
                };
                return vec![Diagnostic {
                    path,
                    span,
                    message,
                }];
            }
            Definition::Function(def) => res.append(&mut def.validate(&path)),
            Definition::Struct(def) => res.append(&mut def.validate(&path)),
        }
        res
    }
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

pub fn find_items(cursor: Cursor) -> Vec<Definition> {
    let mut out = Vec::new();
    for m in cursor.query(vec![FunctionDefinition::query(), StructDefinition::query()]) {
        let def = match m.query_number {
            0 => FunctionDefinition::extract(m),
            1 => StructDefinition::extract(m),
            _ => unreachable!(),
        }
        .unwrap_or_else(Definition::NatspecParsingError);
        out.push(def);
    }
    out
}
