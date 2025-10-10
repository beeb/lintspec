//! Source item definitions
//!
//! This module contains structs for each of the source item types that can be documented with `NatSpec`.
//! The [`Definition`] type provides a unified interface to interact with the various types.
use std::{fmt, ops::Range};

use constructor::ConstructorDefinition;
use contract::ContractDefinition;
use derive_more::{Add, Display, From, IsVariant, TryInto};
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

/// A span of source code
pub type TextRange = Range<TextIndex>;

/// A position inside of the source code
///
/// Lines and columns start at 0.
#[derive(Default, Hash, Copy, Clone, PartialEq, Eq, Debug, Serialize, Add)]
pub struct TextIndex {
    pub utf8: usize,
    pub utf16: usize,
    pub line: usize,
    pub column: usize,
}

impl TextIndex {
    /// Shorthand for `TextIndex { utf8: 0, utf16: 0, line: 0, char: 0 }`.
    pub const ZERO: TextIndex = TextIndex {
        utf8: 0,
        utf16: 0,
        line: 0,
        column: 0,
    };

    /// Advances the index, accounting for lf/nl/ls/ps characters and combinations.
    /// This is *not* derived from the definition of 'newline' in the language definition,
    /// nor is it a complete implementation of the Unicode line breaking algorithm.
    ///
    /// Implementation is directly taken from [`slang_solidity`].
    #[inline]
    pub fn advance(&mut self, c: char, next: Option<&char>) {
        self.utf8 += c.len_utf8();
        self.utf16 += c.len_utf16();
        match (c, next) {
            ('\r', Some('\n')) => {
                // Ignore for now, we will increment the line number whe we process the \n
            }
            ('\n' | '\r' | '\u{2028}' | '\u{2029}', _) => {
                self.line += 1;
                self.column = 0;
            }
            _ => {
                self.column += 1;
            }
        }
    }
}

impl fmt::Display for TextIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line + 1, self.column + 1)
    }
}

impl PartialOrd for TextIndex {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TextIndex {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.utf8.cmp(&other.utf8)
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "lowercase")]
pub enum ContractType {
    #[display("contract")]
    Contract,
    #[display("interface")]
    Interface,
    #[display("library")]
    Library,
}

/// A type of source item (function, struct, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
    #[display("contract")]
    Contract,
    #[display("interface")]
    Interface,
    #[display("library")]
    Library,
    #[display("constructor")]
    Constructor,
    #[display("enum")]
    Enum,
    #[display("error")]
    Error,
    #[display("event")]
    Event,
    #[display("function")]
    PrivateFunction,
    #[display("function")]
    InternalFunction,
    #[display("function")]
    PublicFunction,
    #[display("function")]
    ExternalFunction,
    #[display("modifier")]
    Modifier,
    #[cfg_attr(feature = "cli", value(skip))]
    ParsingError,
    #[display("struct")]
    Struct,
    #[display("variable")]
    PrivateVariable,
    #[display("variable")]
    InternalVariable,
    #[display("variable")]
    PublicVariable,
}
