use std::{fs, path::Path};

use derive_more::From;
use slang_solidity::{
    cst::{Cursor, NonterminalKind, Query, QueryMatch},
    parser::Parser,
};

use crate::{
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

#[derive(Debug, Clone, From)]
pub enum Definition {
    Function(FunctionDefinition),
}

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub args: Vec<String>,
    pub returns: Vec<String>,
    pub comment: Option<String>,
}

pub fn find_items(cursor: Cursor) -> Vec<Definition> {
    let function_query = Query::parse(
        "[FunctionDefinition
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
    let struct_query = Query::parse("[StructDefinition @struct_members members:[StructMembers]]")
        .expect("query should compile");
    for m in cursor.query(vec![function_query, struct_query]) {
        println!("===== new match for query {}", m.query_number);
        match m.query_number {
            0 => {
                let function_name = capture!(m, "function_name");
                let function_name = function_name.node().unparse().trim().to_string();
                println!("Function name: {function_name}");

                let function_params = capture!(m, "function_params");
                let function_params = function_params.node().unparse();
                println!("Function params: {function_params}");

                let function_returns = capture!(m, "function_returns");
                let function_returns = function_returns.node().unparse();
                println!("Function returns: {function_returns}");
            }
            1 => {
                let Some((_, captures)) = m.capture("struct_members") else {
                    continue;
                };
                for capture in captures {
                    println!("new capture");
                    println!("{}", capture.node().unparse().trim());
                }
            }
            _ => unreachable!(),
        }
    }
    vec![]
}
