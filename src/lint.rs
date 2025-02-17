use std::{fs, path::Path};

use derive_more::From;
use slang_solidity::{
    cst::{Cursor, NonterminalKind, Query, TerminalKind, TextRange},
    parser::Parser,
};
use winnow::Parser as _;

use crate::{
    comment::{parse_comment, NatSpec},
    error::{Error, Result},
    utils::detect_solidity_version,
};

macro_rules! capture {
    ($m:ident, $name:expr) => {
        match $m.capture($name).map(|(_, mut captures)| captures.next()) {
            Some(Some(res)) => res,
            _ => {
                continue;
            }
        }
    };
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
}

#[derive(Debug, From)]
pub enum Definition {
    Function(FunctionDefinition),
    Struct(StructDefinition),
    NatspecParsingError(Error),
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: String,
    pub span: TextRange,
}

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub params: Vec<Identifier>,
    pub returns: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub name: String,
    pub members: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
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
    let items = find_items(cursor);
    println!("======================== {:?}", path.as_ref());
    println!(
        "{:#?}",
        items
            .iter()
            .filter(|i| matches!(i, Definition::NatspecParsingError(_)))
            .collect::<Vec<_>>()
    );

    Ok(Vec::new())
}

pub fn find_items(cursor: Cursor) -> Vec<Definition> {
    let function_query = Query::parse(
        "@function [FunctionDefinition
            @function_name name:[FunctionName]
            parameters:[ParametersDeclaration
                @function_params parameters:[Parameters]
            ]
            returns:[
                ReturnsDeclaration variables:[ParametersDeclaration
                    @function_returns parameters:[Parameters]
                ]
            ]
        ]",
    )
    .expect("query should compile");
    let struct_query = Query::parse(
        "@struct [StructDefinition
            @struct_name name:[Identifier]
            @struct_members members:[StructMembers]
        ]",
    )
    .expect("query should compile");
    let mut out = Vec::new();
    for m in cursor.query(vec![function_query, struct_query]) {
        let def = match m.query_number {
            0 => extract_function(
                capture!(m, "function"),
                capture!(m, "function_name"),
                capture!(m, "function_params"),
                capture!(m, "function_returns"),
            ),
            1 => extract_struct(
                capture!(m, "struct"),
                capture!(m, "struct_name"),
                capture!(m, "struct_members"),
            ),
            _ => unreachable!(),
        }
        .unwrap_or_else(Definition::NatspecParsingError);
        out.push(def);
    }
    out
}

fn extract_identifiers(cursor: Cursor) -> Vec<Identifier> {
    let mut cursor = cursor.spawn();
    let mut out = Vec::new();
    while cursor.go_to_next_terminal_with_kind(TerminalKind::Identifier) {
        out.push(Identifier {
            name: cursor.node().unparse(),
            span: cursor.text_range(),
        })
    }
    out
}

fn extract_comment(cursor: Cursor, returns: &[Identifier]) -> Result<Option<NatSpec>> {
    let mut cursor = cursor.spawn();
    let mut items = Vec::new();
    while cursor.go_to_next_terminal_with_kinds(&[
        TerminalKind::MultiLineNatSpecComment,
        TerminalKind::SingleLineNatSpecComment,
    ]) {
        items.push(
            parse_comment
                .parse(&cursor.node().unparse())
                .map_err(|e| Error::NatspecParsingError {
                    span: cursor.text_range(),
                    message: e.to_string(),
                })?
                .populate_returns(returns.iter().map(|r| r.name.as_str())),
        );
    }
    let items = items.into_iter().reduce(|mut acc, mut i| {
        acc.append(&mut i);
        acc
    });
    Ok(items)
}

fn extract_function(
    cursor: Cursor,
    name: Cursor,
    params: Cursor,
    returns: Cursor,
) -> Result<Definition> {
    let name = name.node().unparse();
    let params = extract_identifiers(params);
    let returns = extract_identifiers(returns);
    let natspec = extract_comment(cursor, &returns)?;

    Ok(FunctionDefinition {
        name,
        params,
        returns,
        natspec,
    }
    .into())
}

fn extract_struct(cursor: Cursor, name: Cursor, members: Cursor) -> Result<Definition> {
    let name = name.node().unparse();
    let members = extract_identifiers(members);
    let natspec = extract_comment(cursor, &[])?;

    Ok(StructDefinition {
        name,
        members,
        natspec,
    }
    .into())
}
