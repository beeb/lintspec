//! Source item definitions
//!
//! This module contains structs for each of the source item types that can be documented with `NatSpec`.
//! The [`Definition`] type provides a unified interface to interact with the various types.
use std::ops::Range;

use constructor::ConstructorDefinition;
use derive_more::{Display, From, IsVariant};
use enumeration::EnumDefinition;
use error::ErrorDefinition;
use event::EventDefinition;
use function::FunctionDefinition;
use modifier::ModifierDefinition;
use serde::{Deserialize, Serialize};
use slang_solidity::cst::TextIndex as SlangTextIndex;
use structure::StructDefinition;
use variable::VariableDeclaration;

use crate::{
    error::Error,
    lint::{Diagnostic, ItemDiagnostics, Validate, ValidationOptions},
};

pub mod constructor;
pub mod enumeration;
pub mod error;
pub mod event;
pub mod function;
pub mod modifier;
pub mod structure;
pub mod variable;

/// Source-related information about a [`Definition`]
pub trait SourceItem {
    fn item_type(&self) -> ItemType;

    /// Retrieve the parent contract, interface or library's name
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
#[derive(Default, Hash, Copy, Clone, PartialEq, Eq, Debug, Serialize)]
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

impl From<SlangTextIndex> for TextIndex {
    fn from(value: SlangTextIndex) -> Self {
        Self {
            utf8: value.utf8,
            utf16: value.utf16,
            line: value.line,
            column: value.column,
        }
    }
}

impl From<TextIndex> for SlangTextIndex {
    fn from(value: TextIndex) -> Self {
        Self {
            utf8: value.utf8,
            utf16: value.utf16,
            line: value.line,
            column: value.column,
        }
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
#[derive(Debug, From, IsVariant)]
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

impl PartialEq for Definition {
    /// Compare two definitions based on their underlying node
    ///
    /// If two instances of the same node generate two definitions (due to quantifiers in the query), this can be
    /// used to deduplicate the instances.
    fn eq(&self, other: &Self) -> bool {
        // TODO: is the usage of the span start fine here, or should we instead rely on the `id()` of the node? It would
        // require adding a field in the definition just for that.
        match (self, other) {
            (Self::Constructor(a), Self::Constructor(b)) => a.span.start == b.span.start,
            (Self::Enumeration(a), Self::Enumeration(b)) => a.span.start == b.span.start,
            (Self::Error(a), Self::Error(b)) => a.span.start == b.span.start,
            (Self::Event(a), Self::Event(b)) => a.span.start == b.span.start,
            (Self::Function(a), Self::Function(b)) => a.span.start == b.span.start,
            (Self::Modifier(a), Self::Modifier(b)) => a.span.start == b.span.start,
            (Self::Struct(a), Self::Struct(b)) => a.span.start == b.span.start,
            (Self::Variable(a), Self::Variable(b)) => a.span.start == b.span.start,
            _ => false,
        }
    }
}

impl Definition {
    /// Convert to the inner constructor definition
    #[must_use]
    pub fn to_constructor(self) -> Option<ConstructorDefinition> {
        match self {
            Definition::Constructor(def) => Some(def),
            _ => None,
        }
    }

    /// Convert to the inner enum definition
    #[must_use]
    pub fn to_enum(self) -> Option<EnumDefinition> {
        match self {
            Definition::Enumeration(def) => Some(def),
            _ => None,
        }
    }

    /// Convert to the inner error definition
    #[must_use]
    pub fn to_error(self) -> Option<ErrorDefinition> {
        match self {
            Definition::Error(def) => Some(def),
            _ => None,
        }
    }

    /// Convert to the inner event definition
    #[must_use]
    pub fn to_event(self) -> Option<EventDefinition> {
        match self {
            Definition::Event(def) => Some(def),
            _ => None,
        }
    }

    /// Convert to the inner function definition
    #[must_use]
    pub fn to_function(self) -> Option<FunctionDefinition> {
        match self {
            Definition::Function(def) => Some(def),
            _ => None,
        }
    }

    /// Convert to the inner modifier definition
    #[must_use]
    pub fn to_modifier(self) -> Option<ModifierDefinition> {
        match self {
            Definition::Modifier(def) => Some(def),
            _ => None,
        }
    }

    /// Convert to the inner struct definition
    #[must_use]
    pub fn to_struct(self) -> Option<StructDefinition> {
        match self {
            Definition::Struct(def) => Some(def),
            _ => None,
        }
    }

    /// Convert to the inner variable declaration
    #[must_use]
    pub fn to_variable(self) -> Option<VariableDeclaration> {
        match self {
            Definition::Variable(def) => Some(def),
            _ => None,
        }
    }
    /// Reference to the inner constructor definition
    #[must_use]
    pub fn as_constructor(&self) -> Option<&ConstructorDefinition> {
        match self {
            Definition::Constructor(def) => Some(def),
            _ => None,
        }
    }

    /// Reference to the inner enum definition
    #[must_use]
    pub fn as_enum(&self) -> Option<&EnumDefinition> {
        match self {
            Definition::Enumeration(def) => Some(def),
            _ => None,
        }
    }

    /// Reference to the inner error definition
    #[must_use]
    pub fn as_error(&self) -> Option<&ErrorDefinition> {
        match self {
            Definition::Error(def) => Some(def),
            _ => None,
        }
    }

    /// Reference to the inner event definition
    #[must_use]
    pub fn as_event(&self) -> Option<&EventDefinition> {
        match self {
            Definition::Event(def) => Some(def),
            _ => None,
        }
    }

    /// Reference to the inner function definition
    #[must_use]
    pub fn as_function(&self) -> Option<&FunctionDefinition> {
        match self {
            Definition::Function(def) => Some(def),
            _ => None,
        }
    }

    /// Reference to the inner modifier definition
    #[must_use]
    pub fn as_modifier(&self) -> Option<&ModifierDefinition> {
        match self {
            Definition::Modifier(def) => Some(def),
            _ => None,
        }
    }

    /// Reference to the inner struct definition
    #[must_use]
    pub fn as_struct(&self) -> Option<&StructDefinition> {
        match self {
            Definition::Struct(def) => Some(def),
            _ => None,
        }
    }

    /// Reference to the inner variable declaration
    #[must_use]
    pub fn as_variable(&self) -> Option<&VariableDeclaration> {
        match self {
            Definition::Variable(def) => Some(def),
            _ => None,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
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
    #[value(skip)]
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
