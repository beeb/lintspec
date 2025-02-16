use std::{fs, path::Path};

use derive_more::From;
use slang_solidity::{
    cst::{Cursor, NonterminalKind, Query, TerminalKind},
    parser::Parser,
};

use crate::{
    comment::NatSpec,
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
    for m in cursor.query(vec![function_query, struct_query]) {
        println!("===== new match for query {}", m.query_number);
        match m.query_number {
            0 => {
                let function = capture!(m, "function");
                let mut comments = function.spawn();
                while comments.go_to_next_terminal_with_kinds(&[
                    TerminalKind::MultiLineNatSpecComment,
                    TerminalKind::SingleLineNatSpecComment,
                ]) {
                    println!("Comment: {}", comments.node().unparse());
                }

                let function_name = capture!(m, "function_name");
                let function_name = function_name.node().unparse();
                println!("Function name: {function_name}");

                let function_params = capture!(m, "function_params");
                let function_params = function_params.node().unparse();
                println!("Function params: {function_params}");

                let function_returns = capture!(m, "function_returns");
                let function_returns = function_returns.node().unparse();
                println!("Function returns: {function_returns}");
            }
            1 => {
                let item = capture!(m, "struct");
                let mut comments = item.spawn();
                while comments.go_to_next_terminal_with_kinds(&[
                    TerminalKind::MultiLineNatSpecComment,
                    TerminalKind::SingleLineNatSpecComment,
                ]) {
                    println!("Comment: {}", comments.node().unparse());
                }

                let struct_name = capture!(m, "struct_name");
                let struct_name = struct_name.node().unparse();
                println!("Struct name: {struct_name}");

                let struct_members = capture!(m, "struct_members");
                let struct_members = struct_members.node().unparse();
                println!("Struct members: {struct_members}");
            }
            _ => unreachable!(),
        }
    }
    vec![]
}
