use std::{
    fs,
    path::{Path, PathBuf},
};

use derive_more::From;
use slang_solidity::{
    cst::{Cursor, NonterminalKind, Query, TerminalKind, TextRange},
    parser::Parser,
};
use winnow::Parser as _;

use crate::{
    comment::{parse_comment, NatSpec, NatSpecKind},
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
            Definition::Function(def) => {
                // raise error if no NatSpec is available
                let Some(natspec) = &def.natspec else {
                    return vec![Diagnostic {
                        path,
                        span: def.span.clone(),
                        message: "missing NatSpec".to_string(),
                    }];
                };
                // fallback and receive do not require NatSpec
                if def.name == "receive" || def.name == "fallback" {
                    return vec![];
                }
                // if there is `inheritdoc`, no further validation is required
                if natspec
                    .items
                    .iter()
                    .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
                {
                    return vec![];
                }
                // check params and returns
                for param in &def.params {
                    let Some(name) = &param.name else {
                        // no need to document unused params
                        continue;
                    };
                    let message = match natspec.count_param(param) {
                        0 => format!("@param {} is missing", name),
                        2.. => format!("@param {} is present more than once", name),
                        _ => {
                            continue;
                        }
                    };
                    res.push(Diagnostic {
                        path: path.clone(),
                        span: param.span.clone(),
                        message,
                    })
                }
                let returns_count = natspec.count_all_returns();
                for (idx, ret) in def.returns.iter().enumerate() {
                    let message = match &ret.name {
                        Some(name) => match natspec.count_return(ret) {
                            0 => format!("@return {} is missing", name),
                            2.. => format!("@return {} is present more than once", name),
                            _ => {
                                continue;
                            }
                        },
                        None => {
                            if idx > returns_count - 1 {
                                format!("@return missing for unnamed return #{}", idx + 1)
                            } else {
                                continue;
                            }
                        }
                    };
                    res.push(Diagnostic {
                        path: path.clone(),
                        span: ret.span.clone(),
                        message,
                    })
                }
            }
            Definition::Struct(def) => {}
        }
        res
    }
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: Option<String>,
    pub span: TextRange,
}

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub span: TextRange,
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
            name: Some(cursor.node().unparse()),
            span: cursor.text_range(),
        })
    }
    out
}

fn extract_params(cursor: Cursor) -> Vec<Identifier> {
    let mut cursor = cursor.spawn();
    let mut out = Vec::new();
    while cursor.go_to_next_nonterminal_with_kind(NonterminalKind::Parameter) {
        let mut sub_cursor = cursor.spawn();
        if sub_cursor.go_to_next_terminal_with_kind(TerminalKind::Identifier) {
            out.push(Identifier {
                name: Some(sub_cursor.node().unparse()),
                span: sub_cursor.text_range(),
            });
        } else {
            out.push(Identifier {
                name: None,
                span: cursor.text_range(),
            });
        }
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
        let comment = &cursor.node().unparse();
        items.push(
            parse_comment
                .parse(comment)
                .map_err(|e| Error::NatspecParsingError {
                    span: cursor.text_range(),
                    message: e.to_string(),
                })?
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
    let params = extract_params(params);
    let returns = extract_params(returns);
    let span = cursor.text_range();
    let natspec = extract_comment(cursor, &returns)?;

    Ok(FunctionDefinition {
        name,
        span,
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
