use std::{fs, path::Path};

use derive_more::From;
use slang_solidity::{
    cst::{Cursor, NonterminalKind, Query, TerminalKind},
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

#[derive(Debug, Clone, From)]
pub enum Definition {
    Function(FunctionDefinition),
    Struct(StructDefinition),
}

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub args: Vec<String>,
    pub returns: Vec<String>,
    pub natspec: Option<NatSpec>,
}

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub name: String,
    pub members: Vec<String>,
    pub natspec: Option<NatSpec>,
}

pub fn lint(path: impl AsRef<Path>) -> Result<Vec<Diagnostic>> {
    let contents = fs::read_to_string(path)?;
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
    println!("{items:?}");

    Ok(Vec::new())
}

pub fn find_items(cursor: Cursor) -> Result<Vec<Definition>> {
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
        println!("===== new match for query {}", m.query_number);
        let def = match m.query_number {
            0 => extract_function(
                capture!(m, "function"),
                capture!(m, "function_name"),
                capture!(m, "function_params"),
                capture!(m, "function_returns"),
            )?,
            1 => extract_struct(
                capture!(m, "struct"),
                capture!(m, "struct_name"),
                capture!(m, "struct_members"),
            )?,
            _ => unreachable!(),
        };
        out.push(def);
    }
    Ok(out)
}

fn extract_comment(mut cursor: Cursor, returns: &[&str]) -> Result<Option<NatSpec>> {
    let mut items = Vec::new();
    while cursor.go_to_next_terminal_with_kinds(&[
        TerminalKind::MultiLineNatSpecComment,
        TerminalKind::SingleLineNatSpecComment,
    ]) {
        items.push(
            parse_comment
                .parse(&cursor.node().unparse())
                .map_err(|e| Error::NatspecParsingError(e.to_string()))?
                .populate_returns(returns),
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
    println!("Function name: {name}");

    let params = params.node().unparse();
    println!("Function params: {params}");

    let returns = returns.node().unparse();
    println!("Function returns: {returns}");

    let natspec = extract_comment(cursor.spawn(), &[])?; // TODO: parse returns

    Ok(FunctionDefinition {
        name,
        args: vec![],
        returns: vec![],
        natspec,
    }
    .into())
}

fn extract_struct(cursor: Cursor, name: Cursor, members: Cursor) -> Result<Definition> {
    let mut comments = cursor.spawn();
    while comments.go_to_next_terminal_with_kinds(&[
        TerminalKind::MultiLineNatSpecComment,
        TerminalKind::SingleLineNatSpecComment,
    ]) {
        println!("Comment: {}", comments.node().unparse());
    }

    let name = name.node().unparse();
    println!("Function name: {name}");

    let members = members.node().unparse();
    println!("Function params: {members}");

    let natspec = extract_comment(cursor.spawn(), &[])?; // TODO: parse returns

    Ok(StructDefinition {
        name,
        members: vec![],
        natspec,
    }
    .into())
}
