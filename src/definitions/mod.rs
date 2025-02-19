use constructor::ConstructorDefinition;
use derive_more::{Display, From};
use enumeration::EnumDefinition;
use error::ErrorDefinition;
use event::EventDefinition;
use function::FunctionDefinition;
use modifier::ModifierDefinition;
use serde::Serialize;
use slang_solidity::cst::{Cursor, NonterminalKind, Query, QueryMatch, TerminalKind, TextRange};
use structure::StructDefinition;
use variable::VariableDeclaration;
use winnow::Parser as _;

use crate::{
    comment::{parse_comment, NatSpec},
    config::Config,
    error::{Error, Result},
    lint::{Diagnostic, ItemDiagnostics, ItemType},
};

pub mod constructor;
pub mod enumeration;
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

#[derive(Debug, Clone)]
pub struct ValidationOptions {
    pub inheritdoc: bool,
    pub constructor: bool,
    pub enum_params: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            inheritdoc: true,
            constructor: false,
            enum_params: false,
        }
    }
}

impl From<&Config> for ValidationOptions {
    fn from(value: &Config) -> Self {
        Self {
            inheritdoc: value.inheritdoc,
            constructor: value.constructor,
            enum_params: value.enum_params,
        }
    }
}

pub trait Validate {
    fn query() -> Query;
    fn extract(m: QueryMatch) -> Result<Definition>;
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics;
    fn parent(&self) -> Option<Parent>;
    fn name(&self) -> String;
    fn span(&self) -> TextRange;
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: Option<String>,
    pub span: TextRange,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Visibility {
    External,
    #[default]
    Internal,
    Private,
    Public,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Attributes {
    pub visibility: Visibility,
    pub r#override: bool,
}

#[derive(Debug, Clone, Display, Serialize)]
#[serde(untagged)]
pub enum Parent {
    Contract(String),
    Interface(String),
    Library(String),
}

#[derive(Debug, From)]
pub enum Definition {
    Constructor(ConstructorDefinition),
    Enumeration(EnumDefinition),
    Error(ErrorDefinition),
    Event(EventDefinition),
    Function(FunctionDefinition),
    Modifier(ModifierDefinition),
    Struct(StructDefinition),
    Variable(VariableDeclaration),
    NatspecParsingError(Error),
}

impl Definition {
    pub fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        match self {
            Definition::NatspecParsingError(error) => {
                let (parent, span, message) = match error {
                    Error::NatspecParsingError {
                        parent,
                        span,
                        message,
                    } => (parent.clone(), span.clone(), message.clone()),
                    _ => (None, TextRange::default(), error.to_string()),
                };
                ItemDiagnostics {
                    parent,
                    item_type: ItemType::ParsingError,
                    name: String::new(),
                    span: span.clone(),
                    diags: vec![Diagnostic { span, message }],
                }
            }
            Definition::Constructor(def) => def.validate(options),
            Definition::Enumeration(def) => def.validate(options),
            Definition::Error(def) => def.validate(options),
            Definition::Event(def) => def.validate(options),
            Definition::Function(def) => def.validate(options),
            Definition::Modifier(def) => def.validate(options),
            Definition::Struct(def) => def.validate(options),
            Definition::Variable(def) => def.validate(options),
        }
    }
}

pub fn find_items(cursor: Cursor) -> Vec<Definition> {
    let mut out = Vec::new();
    for m in cursor.query(vec![
        ConstructorDefinition::query(),
        EnumDefinition::query(),
        ErrorDefinition::query(),
        EventDefinition::query(),
        FunctionDefinition::query(),
        ModifierDefinition::query(),
        StructDefinition::query(),
        VariableDeclaration::query(),
    ]) {
        let def = match m.query_number {
            0 => ConstructorDefinition::extract(m),
            1 => EnumDefinition::extract(m),
            2 => ErrorDefinition::extract(m),
            3 => EventDefinition::extract(m),
            4 => FunctionDefinition::extract(m),
            5 => ModifierDefinition::extract(m),
            6 => StructDefinition::extract(m),
            7 => VariableDeclaration::extract(m),
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
                name: Some(sub_cursor.node().unparse().trim().to_string()),
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
                    parent: parent_contract_name(cursor.clone()),
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
            name: Some(cursor.node().unparse().trim().to_string()),
            span: cursor.text_range(),
        })
    }
    out
}

pub fn check_params(natspec: &NatSpec, params: &[Identifier]) -> Vec<Diagnostic> {
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
            span: param.span.clone(),
            message,
        })
    }
    res
}

pub fn check_returns(natspec: &NatSpec, returns: &[Identifier]) -> Vec<Diagnostic> {
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
            span: ret.span.clone(),
            message,
        })
    }
    res
}

pub fn parent_contract_name(mut cursor: Cursor) -> Option<Parent> {
    while cursor.go_to_parent() {
        if let Some(parent) = cursor.node().as_nonterminal_with_kinds(&[
            NonterminalKind::ContractDefinition,
            NonterminalKind::InterfaceDefinition,
            NonterminalKind::LibraryDefinition,
        ]) {
            for child in &parent.children {
                if child.is_terminal_with_kind(TerminalKind::Identifier) {
                    let name = child.node.unparse().trim().to_string();
                    return Some(match parent.kind {
                        NonterminalKind::ContractDefinition => Parent::Contract(name),
                        NonterminalKind::InterfaceDefinition => Parent::Interface(name),
                        NonterminalKind::LibraryDefinition => Parent::Library(name),
                        _ => unreachable!(),
                    });
                }
            }
        }
    }
    None
}

pub fn get_attributes(cursor: Cursor) -> Attributes {
    let mut cursor = cursor.spawn();
    let mut out = Attributes::default();
    while cursor.go_to_next_terminal_with_kinds(&[
        TerminalKind::ExternalKeyword,
        TerminalKind::InternalKeyword,
        TerminalKind::PrivateKeyword,
        TerminalKind::PublicKeyword,
        TerminalKind::OverrideKeyword,
    ]) {
        match cursor
            .node()
            .as_terminal()
            .expect("should be terminal kind")
            .kind
        {
            TerminalKind::ExternalKeyword => out.visibility = Visibility::External,
            TerminalKind::InternalKeyword => out.visibility = Visibility::Internal,
            TerminalKind::PrivateKeyword => out.visibility = Visibility::Private,
            TerminalKind::PublicKeyword => out.visibility = Visibility::Public,
            TerminalKind::OverrideKeyword => out.r#override = true,
            _ => unreachable!(),
        }
    }
    out
}
