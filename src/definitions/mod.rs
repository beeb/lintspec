use derive_more::From;
use error::ErrorDefinition;
use event::EventDefinition;
use function::FunctionDefinition;
use modifier::ModifierDefinition;
use slang_solidity::cst::{Cursor, NonterminalKind, Query, QueryMatch, TerminalKind, TextRange};
use structure::StructDefinition;
use variable::VariableDeclaration;
use winnow::Parser as _;

use crate::{
    comment::{parse_comment, NatSpec},
    error::{Error, Result},
    lint::{CheckType, Diagnostic},
};

pub mod error;
pub mod event;
pub mod function;
pub mod modifier;
pub mod structure;
pub mod variable;

macro_rules! capture {
    ($m:ident, $name:expr) => {
        match $m.capture($name).map(|(_, mut captures)| captures.next()) {
            Some(Some(res)) => res,
            _ => {
                return Err($crate::error::Error::UnknownError);
            }
        }
    };
}

pub(crate) use capture;

pub trait Validate {
    fn query() -> Query;
    fn extract(m: QueryMatch) -> Result<Definition>;
    fn validate(&self) -> Vec<Diagnostic>;
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: Option<String>,
    pub span: TextRange,
}

#[derive(Debug, From)]
pub enum Definition {
    Error(ErrorDefinition),
    Event(EventDefinition),
    Function(FunctionDefinition),
    Modifier(ModifierDefinition),
    Struct(StructDefinition),
    Variable(VariableDeclaration),
    NatspecParsingError(Error),
}

impl Definition {
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut res = Vec::new();
        match self {
            Definition::NatspecParsingError(error) => {
                let (span, message) = match error {
                    Error::NatspecParsingError { span, message } => (span.clone(), message.clone()),
                    _ => (TextRange::default(), error.to_string()),
                };
                return vec![Diagnostic {
                    check_type: CheckType::ParsingError,
                    span,
                    message,
                }];
            }
            Definition::Error(def) => res.append(&mut def.validate()),
            Definition::Event(def) => res.append(&mut def.validate()),
            Definition::Function(def) => res.append(&mut def.validate()),
            Definition::Modifier(def) => res.append(&mut def.validate()),
            Definition::Struct(def) => res.append(&mut def.validate()),
            Definition::Variable(def) => res.append(&mut def.validate()),
        }
        res
    }
}

pub fn find_items(cursor: Cursor) -> Vec<Definition> {
    let mut out = Vec::new();
    for m in cursor.query(vec![
        ErrorDefinition::query(),
        EventDefinition::query(),
        FunctionDefinition::query(),
        ModifierDefinition::query(),
        StructDefinition::query(),
        VariableDeclaration::query(),
    ]) {
        let def = match m.query_number {
            0 => ErrorDefinition::extract(m),
            1 => EventDefinition::extract(m),
            2 => FunctionDefinition::extract(m),
            3 => ModifierDefinition::extract(m),
            4 => StructDefinition::extract(m),
            5 => VariableDeclaration::extract(m),
            _ => unreachable!(),
        }
        .unwrap_or_else(Definition::NatspecParsingError);
        out.push(def);
    }
    out
}

pub fn extract_params(cursor: Cursor, kind: NonterminalKind) -> Vec<Identifier> {
    let mut cursor = cursor.spawn();
    let mut out = Vec::new();
    while cursor.go_to_next_nonterminal_with_kind(kind) {
        let mut sub_cursor = cursor.spawn().with_edges();
        let mut found = false;
        while sub_cursor.go_to_next_terminal_with_kind(TerminalKind::Identifier) {
            if let Some(label) = sub_cursor.label() {
                if label.to_string() != "name" {
                    continue;
                }
            }
            found = true;
            out.push(Identifier {
                name: Some(sub_cursor.node().unparse()),
                span: sub_cursor.text_range(),
            });
        }
        if !found {
            out.push(Identifier {
                name: None,
                span: cursor.text_range(),
            });
        }
    }
    out
}

pub fn extract_comment(cursor: Cursor, returns: &[Identifier]) -> Result<Option<NatSpec>> {
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

pub fn extract_identifiers(cursor: Cursor) -> Vec<Identifier> {
    let mut cursor = cursor.spawn().with_edges();
    let mut out = Vec::new();
    while cursor.go_to_next_terminal_with_kind(TerminalKind::Identifier) {
        if let Some(label) = cursor.label() {
            if label.to_string() != "name" {
                continue;
            }
        }
        out.push(Identifier {
            name: Some(cursor.node().unparse()),
            span: cursor.text_range(),
        })
    }
    out
}

pub fn check_params(
    natspec: &NatSpec,
    params: &[Identifier],
    check_type: CheckType,
) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    for param in params {
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
            check_type,
            span: param.span.clone(),
            message,
        })
    }
    res
}

pub fn check_returns(
    natspec: &NatSpec,
    returns: &[Identifier],
    check_type: CheckType,
) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    let returns_count = returns.len();
    for (idx, ret) in returns.iter().enumerate() {
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
            check_type,
            span: ret.span.clone(),
            message,
        })
    }
    res
}
