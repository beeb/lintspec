//! Source item definitions
//!
//! This module contains structs for each of the source item types that can be documented with `NatSpec`.
//! The [`Definition`] type provides a unified interface to interact with the various types.
use constructor::ConstructorDefinition;
use contract::ContractDefinition;
use derive_more::{Display, From, FromStr, IsVariant, TryInto};
use enumeration::EnumDefinition;
use error::ErrorDefinition;
use event::EventDefinition;
use function::FunctionDefinition;
use lintspec_macros::AsToVariant;
use modifier::ModifierDefinition;
use serde::{Deserialize, Serialize};
use structure::StructDefinition;
use variable::VariableDeclaration;

use crate::{
    definitions::{interface::InterfaceDefinition, library::LibraryDefinition},
    error::Error,
    lint::{Diagnostic, ItemDiagnostics, Validate, ValidationOptions},
    textindex::TextRange,
};

pub mod constructor;
pub mod contract;
pub mod enumeration;
pub mod error;
pub mod event;
pub mod function;
pub mod interface;
pub mod library;
pub mod modifier;
pub mod structure;
pub mod variable;

/// Source-related information about a [`Definition`]
pub trait SourceItem {
    fn item_type(&self) -> ItemType;

    /// Retrieve the parent contract, interface, or library's name
    fn parent(&self) -> Option<Parent>;

    /// Retrieve the name of the source item
    fn name(&self) -> String;

    /// Retrieve the span of the source item
    fn span(&self) -> TextRange;
}

/// An identifier (named or unnamed) and its span
///
/// Unnamed identifiers are used for unnamed return params.
#[derive(Debug, Clone, bon::Builder)]
#[builder(on(String, into))]
pub struct Identifier {
    pub name: Option<String>,
    pub span: TextRange,
}

/// The visibility modifier for a function-like item
///
/// If no modifier is present, the default is `Internal`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Visibility {
    External,
    #[default]
    Internal,
    Private,
    Public,
}

/// Attributes for a function or state variable (visibility and override)
#[derive(Debug, Clone, Copy, Default, bon::Builder)]
#[non_exhaustive]
pub struct Attributes {
    pub visibility: Visibility,
    pub r#override: bool,
}

/// The name and type of a source item's parent
#[derive(Debug, Clone, Display, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Parent {
    Contract(String),
    Interface(String),
    Library(String),
}

/// A source item's definition
#[derive(Debug, From, TryInto, IsVariant, AsToVariant)]
pub enum Definition {
    Contract(ContractDefinition),
    Interface(InterfaceDefinition),
    Library(LibraryDefinition),
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

impl PartialEq for Definition {
    /// Compare two definitions based on their underlying node
    ///
    /// If two instances of the same node generate two definitions (due to quantifiers in the query), this can be
    /// used to deduplicate the instances.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Contract(a), Self::Contract(b)) => a.span.start == b.span.start,
            (Self::Interface(a), Self::Interface(b)) => a.span.start == b.span.start,
            (Self::Library(a), Self::Library(b)) => a.span.start == b.span.start,
            (Self::Constructor(a), Self::Constructor(b)) => a.span.start == b.span.start,
            (Self::Enumeration(a), Self::Enumeration(b)) => a.span.start == b.span.start,
            (Self::Error(a), Self::Error(b)) => a.span.start == b.span.start,
            (Self::Event(a), Self::Event(b)) => a.span.start == b.span.start,
            (Self::Function(a), Self::Function(b)) => a.span.start == b.span.start,
            (Self::Modifier(a), Self::Modifier(b)) => a.span.start == b.span.start,
            (Self::Struct(a), Self::Struct(b)) => a.span.start == b.span.start,
            (Self::Variable(a), Self::Variable(b)) => a.span.start == b.span.start,
            (
                Self::NatspecParsingError(Error::NatspecParsingError { span: span_a, .. }),
                Self::NatspecParsingError(Error::NatspecParsingError { span: span_b, .. }),
            ) => span_a.start == span_b.start,
            (Self::NatspecParsingError(a), Self::NatspecParsingError(b)) => {
                a.to_string() == b.to_string() // try to compare on error message
            }
            _ => false,
        }
    }
}

impl Definition {
    /// Retrieve the span of a definition
    #[must_use]
    pub fn span(&self) -> Option<TextRange> {
        match self {
            Definition::Contract(d) => Some(d.span()),
            Definition::Interface(d) => Some(d.span()),
            Definition::Library(d) => Some(d.span()),
            Definition::Constructor(d) => Some(d.span()),
            Definition::Enumeration(d) => Some(d.span()),
            Definition::Error(d) => Some(d.span()),
            Definition::Event(d) => Some(d.span()),
            Definition::Function(d) => Some(d.span()),
            Definition::Modifier(d) => Some(d.span()),
            Definition::Struct(d) => Some(d.span()),
            Definition::Variable(d) => Some(d.span()),
            Definition::NatspecParsingError(Error::NatspecParsingError { span, .. }) => {
                Some(span.clone())
            }
            Definition::NatspecParsingError(_) => None,
        }
    }

    /// Mutably borrow the span of a definition
    pub fn span_mut(&mut self) -> Option<&mut TextRange> {
        match self {
            Definition::Contract(d) => Some(&mut d.span),
            Definition::Interface(d) => Some(&mut d.span),
            Definition::Library(d) => Some(&mut d.span),
            Definition::Constructor(d) => Some(&mut d.span),
            Definition::Enumeration(d) => Some(&mut d.span),
            Definition::Error(d) => Some(&mut d.span),
            Definition::Event(d) => Some(&mut d.span),
            Definition::Function(d) => Some(&mut d.span),
            Definition::Modifier(d) => Some(&mut d.span),
            Definition::Struct(d) => Some(&mut d.span),
            Definition::Variable(d) => Some(&mut d.span),
            Definition::NatspecParsingError(Error::NatspecParsingError { span, .. }) => Some(span),
            Definition::NatspecParsingError(_) => None,
        }
    }
}

impl Validate for Definition {
    /// Validate a definition and generate [`Diagnostic`]s for errors
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        match self {
            // if there was an error while parsing the NatSpec comments, a special diagnostic is generated
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
            Definition::Contract(def) => def.validate(options),
            Definition::Interface(def) => def.validate(options),
            Definition::Library(def) => def.validate(options),
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

/// A type of source item (function, struct, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, FromStr)]
#[serde(rename_all = "snake_case")]
#[display(rename_all = "snake_case")]
pub enum ContractType {
    Contract,
    Interface,
    Library,
}

/// A type of source item (function, struct, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, FromStr)]
#[serde(rename_all = "snake_case")]
#[display(rename_all = "snake_case")]
#[from_str(rename_all = "snake_case")]
pub enum ItemType {
    Contract,
    Interface,
    Library,
    Constructor,
    Enum,
    Error,
    Event,
    #[display("function")]
    PrivateFunction,
    #[display("function")]
    InternalFunction,
    #[display("function")]
    PublicFunction,
    #[display("function")]
    ExternalFunction,
    Modifier,
    ParsingError,
    Struct,
    #[display("variable")]
    PrivateVariable,
    #[display("variable")]
    InternalVariable,
    #[display("variable")]
    PublicVariable,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_contract_type_display() {
        assert_eq!(ContractType::Contract.to_string(), "contract");
        assert_eq!(ContractType::Interface.to_string(), "interface");
        assert_eq!(ContractType::Library.to_string(), "library");
    }

    #[test]
    fn test_contract_type_from_str() {
        assert_eq!(
            ContractType::from_str("contract").unwrap(),
            ContractType::Contract
        );
        assert_eq!(
            ContractType::from_str("interface").unwrap(),
            ContractType::Interface
        );
        assert_eq!(
            ContractType::from_str("library").unwrap(),
            ContractType::Library
        );
    }

    #[test]
    fn test_contract_type_from_str_case_insensitive() {
        // FromStr is case-insensitive
        assert_eq!(
            ContractType::from_str("Contract").unwrap(),
            ContractType::Contract
        );
        assert_eq!(
            ContractType::from_str("INTERFACE").unwrap(),
            ContractType::Interface
        );
        assert_eq!(
            ContractType::from_str("Library").unwrap(),
            ContractType::Library
        );
    }

    #[test]
    fn test_contract_type_from_str_invalid() {
        assert!(ContractType::from_str("invalid").is_err());
        assert!(ContractType::from_str("").is_err());
    }

    #[test]
    fn test_item_type_display() {
        assert_eq!(ItemType::Contract.to_string(), "contract");
        assert_eq!(ItemType::Interface.to_string(), "interface");
        assert_eq!(ItemType::Library.to_string(), "library");
        assert_eq!(ItemType::Constructor.to_string(), "constructor");
        assert_eq!(ItemType::Enum.to_string(), "enum");
        assert_eq!(ItemType::Error.to_string(), "error");
        assert_eq!(ItemType::Event.to_string(), "event");
        assert_eq!(ItemType::Modifier.to_string(), "modifier");
        assert_eq!(ItemType::ParsingError.to_string(), "parsing_error");
        assert_eq!(ItemType::Struct.to_string(), "struct");
        assert_eq!(ItemType::PrivateFunction.to_string(), "function");
        assert_eq!(ItemType::InternalFunction.to_string(), "function");
        assert_eq!(ItemType::PublicFunction.to_string(), "function");
        assert_eq!(ItemType::ExternalFunction.to_string(), "function");
        assert_eq!(ItemType::PrivateVariable.to_string(), "variable");
        assert_eq!(ItemType::InternalVariable.to_string(), "variable");
        assert_eq!(ItemType::PublicVariable.to_string(), "variable");
    }

    #[test]
    fn test_item_type_from_str() {
        assert_eq!(ItemType::from_str("contract").unwrap(), ItemType::Contract);
        assert_eq!(
            ItemType::from_str("interface").unwrap(),
            ItemType::Interface
        );
        assert_eq!(ItemType::from_str("library").unwrap(), ItemType::Library);
        assert_eq!(
            ItemType::from_str("constructor").unwrap(),
            ItemType::Constructor
        );
        assert_eq!(ItemType::from_str("enum").unwrap(), ItemType::Enum);
        assert_eq!(ItemType::from_str("error").unwrap(), ItemType::Error);
        assert_eq!(ItemType::from_str("event").unwrap(), ItemType::Event);
        assert_eq!(ItemType::from_str("modifier").unwrap(), ItemType::Modifier);
        assert_eq!(
            ItemType::from_str("parsing_error").unwrap(),
            ItemType::ParsingError
        );
        assert_eq!(ItemType::from_str("struct").unwrap(), ItemType::Struct);
        assert_eq!(
            ItemType::from_str("private_function").unwrap(),
            ItemType::PrivateFunction
        );
        assert_eq!(
            ItemType::from_str("internal_function").unwrap(),
            ItemType::InternalFunction
        );
        assert_eq!(
            ItemType::from_str("public_function").unwrap(),
            ItemType::PublicFunction
        );
        assert_eq!(
            ItemType::from_str("external_function").unwrap(),
            ItemType::ExternalFunction
        );
        assert_eq!(
            ItemType::from_str("private_variable").unwrap(),
            ItemType::PrivateVariable
        );
        assert_eq!(
            ItemType::from_str("internal_variable").unwrap(),
            ItemType::InternalVariable
        );
        assert_eq!(
            ItemType::from_str("public_variable").unwrap(),
            ItemType::PublicVariable
        );
    }

    #[test]
    fn test_item_type_from_str_case_sensitive() {
        assert!(ItemType::from_str("Contract").is_err());
        assert!(ItemType::from_str("PRIVATE_FUNCTION").is_err());
        assert!(ItemType::from_str("PrivateFunction").is_err());
    }

    #[test]
    fn test_item_type_from_str_invalid() {
        assert!(ItemType::from_str("invalid").is_err());
        assert!(ItemType::from_str("function").is_err()); // ambiguous
        assert!(ItemType::from_str("variable").is_err()); // ambiguous
        assert!(ItemType::from_str("").is_err());
    }
}
