//! Parsing and validation of source item definitions
//!
//! This module contains structs for each of the source item types that can be documented with `NatSpec`.
//! The [`Definition`] type provides a unified interface to interact with the various types.
//! The [`find_items`] function takes the tree root [`Cursor`] from a Solidity document and returns all the parsed
//! definitions contained within.
//! Some helper functions allow to extract useful information from [`Cursor`]s.
use constructor::ConstructorDefinition;
use derive_more::{Display, From, IsVariant};
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
    config::Config,
    error::{Error, Result},
    lint::{Diagnostic, ItemDiagnostics, ItemType},
    natspec::{parse_comment, NatSpec},
};

pub mod constructor;
pub mod enumeration;
pub mod error;
pub mod event;
pub mod function;
pub mod modifier;
pub mod structure;
pub mod variable;

/// Retrieve and unwrap the first capture of a parser match, or return with an [`Error`]
pub fn capture(m: &QueryMatch, name: &str) -> Result<Cursor> {
    match m.capture(name).map(|(_, mut captures)| captures.next()) {
        Some(Some(res)) => Ok(res),
        _ => Err(Error::UnknownError),
    }
}

/// Retrieve and unwrap the first capture of a parser match if one exists.
pub fn capture_opt(m: &QueryMatch, name: &str) -> Result<Option<Cursor>> {
    match m.capture(name).map(|(_, mut captures)| captures.next()) {
        Some(Some(res)) => Ok(Some(res)),
        Some(None) => Ok(None),
        _ => Err(Error::UnknownError),
    }
}

/// Validation options to control which lints generate a diagnostic
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)]
pub struct ValidationOptions {
    /// Whether overridden, public and external functions should have an `@inheritdoc`
    #[builder(default = true)]
    pub inheritdoc: bool,

    /// Whether to check that constructors have documented params
    #[builder(default = false)]
    pub constructor: bool,

    /// Whether to check that each member of structs is documented with `@param`
    ///
    /// Not standard practice, the Solidity spec does not consider `@param` for structs or provide any other way to
    /// document each member.
    #[builder(default = false)]
    pub struct_params: bool,

    /// Whether to check that each variant of enums is documented with `@param`
    ///
    /// Not standard practice, the Solidity spec does not consider `@param` for enums or provide any other way to
    /// document each variant.
    #[builder(default = false)]
    pub enum_params: bool,

    /// Which item types should have at least some [`NatSpec`] even if they have no param/return/member.
    #[builder(default = vec![])]
    pub enforce: Vec<ItemType>,
}

impl Default for ValidationOptions {
    /// Get default validation options (`inheritdoc` is true, the others are false)
    fn default() -> Self {
        Self {
            inheritdoc: true,
            constructor: false,
            struct_params: false,
            enum_params: false,
            enforce: vec![],
        }
    }
}

impl From<&Config> for ValidationOptions {
    fn from(value: &Config) -> Self {
        Self {
            inheritdoc: value.inheritdoc,
            constructor: value.constructor,
            struct_params: value.struct_params,
            enum_params: value.enum_params,
            enforce: value.enforce.clone(),
        }
    }
}

/// A trait implemented by the various source item definitions
pub trait Validate {
    /// Return a [`slang_solidity`] [`Query`] used to extract information about the source item
    fn query() -> Query;

    /// Extract information from the query matches
    fn extract(m: QueryMatch) -> Result<Definition>;

    /// Validate the definition and extract the relevant diagnostics
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics;

    /// Retrieve the parent contract, interface or library's name
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
    /// Validate a definition and generate [`Diagnostic`]s for errors
    #[must_use]
    pub fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
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

/// Find source item definitions from a root CST [`Cursor`]
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
            0 => Some(
                ConstructorDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError),
            ),
            1 => Some(EnumDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError)),
            2 => Some(ErrorDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError)),
            3 => Some(EventDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError)),
            4 => {
                let def =
                    FunctionDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError);
                if out.contains(&def) {
                    None
                } else {
                    Some(def)
                }
            }
            5 => {
                let def =
                    ModifierDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError);
                if out.contains(&def) {
                    None
                } else {
                    Some(def)
                }
            }
            6 => Some(StructDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError)),
            7 => Some(
                VariableDeclaration::extract(m).unwrap_or_else(Definition::NatspecParsingError),
            ),
            _ => unreachable!(),
        };
        if let Some(def) = def {
            out.push(def);
        }
    }
    out
}

/// Extract parameters from a function-like source item.
///
/// The node kind that holds the `Identifier` (`Parameter`, `EventParameter`) must be provided with `kind`.
#[must_use]
pub fn extract_params(cursor: &Cursor, kind: NonterminalKind) -> Vec<Identifier> {
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

/// Extract and parse the [`NatSpec`] comment information, if any
pub fn extract_comment(cursor: &Cursor, returns: &[Identifier]) -> Result<Option<NatSpec>> {
    let mut cursor = cursor.spawn();
    let mut items = Vec::new();
    while cursor.go_to_next() {
        if cursor.node().is_terminal_with_kinds(&[
            TerminalKind::MultiLineNatSpecComment,
            TerminalKind::SingleLineNatSpecComment,
        ]) {
            let comment = &cursor.node().unparse();
            items.push((
                cursor.node().kind().to_string(), // the node type to differentiate multiline for single line
                cursor.text_range().start.line, // the line number to remove unwanted single-line comments
                parse_comment
                    .parse(comment)
                    .map_err(|e| Error::NatspecParsingError {
                        parent: extract_parent_name(cursor.clone()),
                        span: cursor.text_range(),
                        message: e.to_string(),
                    })?
                    .populate_returns(returns),
            ));
        } else if cursor.node().is_terminal_with_kinds(&[
            TerminalKind::ConstructorKeyword,
            TerminalKind::EnumKeyword,
            TerminalKind::ErrorKeyword,
            TerminalKind::EventKeyword,
            TerminalKind::FunctionKeyword,
            TerminalKind::ModifierKeyword,
            TerminalKind::StructKeyword,
        ]) | cursor
            .node()
            .is_nonterminal_with_kind(NonterminalKind::StateVariableAttributes)
        {
            // anything after this node should be ignored, because we enter the item's body
            break;
        }
    }
    if let Some("MultiLineNatSpecComment") = items.last().map(|(kind, _, _)| kind.as_str()) {
        // if the last comment is multiline, we ignore all previous comments
        let (_, _, natspec) = items.pop().expect("there should be at least one elem");
        return Ok(Some(natspec));
    }
    // the last comment is single-line
    // we need to take the comments (in reverse) up to an empty line or a multiline comment (exclusive)
    let mut res = Vec::new();
    let mut iter = items.into_iter().rev().peekable();
    while let Some((_, item_line, item)) = iter.next() {
        res.push(item);
        if let Some((next_kind, next_line, _)) = iter.peek() {
            if next_kind == "MultiLineNatSpecComment" || *next_line < item_line - 1 {
                // the next comments up should be ignored
                break;
            }
        }
    }
    if res.is_empty() {
        return Ok(None);
    }
    Ok(Some(res.into_iter().rev().fold(
        NatSpec::default(),
        |mut acc, mut i| {
            acc.append(&mut i);
            acc
        },
    )))
}

/// Extract identifiers from a CST node, filtered by label equal to `name`
#[must_use]
pub fn extract_identifiers(cursor: &Cursor) -> Vec<Identifier> {
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
        });
    }
    out
}

/// Extract the attributes (visibility and override) from a function-like item or state variable
#[must_use]
pub fn extract_attributes(cursor: &Cursor) -> Attributes {
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

/// Find the parent's name (contract, interface, library), if any
#[must_use]
pub fn extract_parent_name(mut cursor: Cursor) -> Option<Parent> {
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

/// Check a list of params to see if they are documented with a corresponding item in the [`NatSpec`], and generate a
/// diagnostic for each missing one or if there are more than 1 entry per param.
#[must_use]
pub fn check_params(natspec: &NatSpec, params: &[Identifier]) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    for param in params {
        let Some(name) = &param.name else {
            // no need to document unused params
            continue;
        };
        let message = match natspec.count_param(param) {
            0 => format!("@param {name} is missing"),
            2.. => format!("@param {name} is present more than once"),
            _ => {
                continue;
            }
        };
        res.push(Diagnostic {
            span: param.span.clone(),
            message,
        });
    }
    res
}

/// Check a list of returns to see if they are documented with a corresponding item in the [`NatSpec`], and generate a
/// diagnostic for each missing one or if there are more than 1 entry per param.
#[allow(clippy::cast_possible_wrap)]
#[must_use]
pub fn check_returns(
    natspec: &NatSpec,
    returns: &[Identifier],
    default_span: TextRange,
) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    let mut unnamed_returns = 0;
    let returns_count = natspec.count_all_returns() as isize;
    for (idx, ret) in returns.iter().enumerate() {
        let message = if let Some(name) = &ret.name {
            match natspec.count_return(ret) {
                0 => format!("@return {name} is missing"),
                2.. => format!("@return {name} is present more than once"),
                _ => {
                    continue;
                }
            }
        } else {
            unnamed_returns += 1;
            if idx as isize > returns_count - 1 {
                format!("@return missing for unnamed return #{}", idx + 1)
            } else {
                continue;
            }
        };
        res.push(Diagnostic {
            span: ret.span.clone(),
            message,
        });
    }
    if natspec.count_unnamed_returns() > unnamed_returns {
        res.push(Diagnostic {
            span: returns.last().cloned().map_or(default_span, |r| r.span),
            message: "too many unnamed returns".to_string(),
        });
    }
    res
}

#[cfg(test)]
mod tests {
    use similar_asserts::assert_eq;
    use slang_solidity::parser::Parser;

    use crate::{
        natspec::{NatSpecItem, NatSpecKind},
        utils::detect_solidity_version,
    };

    use super::*;

    fn parse_file(contents: &str) -> Cursor {
        let solidity_version = detect_solidity_version(contents).unwrap();
        let parser = Parser::create(solidity_version).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        output.create_tree_cursor()
    }

    macro_rules! impl_find_item {
        ($fn_name:ident, $item_variant:path, $item_type:ty) => {
            fn $fn_name<'a>(
                name: &str,
                parent: Option<Parent>,
                items: &'a [Definition],
            ) -> &'a $item_type {
                items
                    .iter()
                    .find_map(|d| match d {
                        $item_variant(ref def) if def.name == name && def.parent == parent => {
                            Some(def)
                        }
                        _ => None,
                    })
                    .unwrap()
            }
        };
    }

    impl_find_item!(find_function, Definition::Function, FunctionDefinition);
    impl_find_item!(find_variable, Definition::Variable, VariableDeclaration);
    impl_find_item!(find_modifier, Definition::Modifier, ModifierDefinition);
    impl_find_item!(find_error, Definition::Error, ErrorDefinition);
    impl_find_item!(find_event, Definition::Event, EventDefinition);
    impl_find_item!(find_struct, Definition::Struct, StructDefinition);

    #[test]
    fn test_parse_external_function() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "viewFunctionNoParams",
            Some(Parent::Contract("ParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Inheritdoc {
                        parent: "IParserTest".to_string()
                    },
                    comment: String::new()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "Dev comment for the function".to_string()
                }
            ]
        );
    }

    #[test]
    fn test_parse_constant() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_variable(
            "SOME_CONSTANT",
            Some(Parent::Contract("ParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![NatSpecItem {
                kind: NatSpecKind::Inheritdoc {
                    parent: "IParserTest".to_string()
                },
                comment: String::new()
            },]
        );
    }

    #[test]
    fn test_parse_variable() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_variable(
            "someVariable",
            Some(Parent::Contract("ParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![NatSpecItem {
                kind: NatSpecKind::Inheritdoc {
                    parent: "IParserTest".to_string()
                },
                comment: String::new()
            },]
        );
    }

    #[test]
    fn test_parse_modifier() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_modifier(
            "someModifier",
            Some(Parent::Contract("ParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "The description of the modifier".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_param1".to_string()
                    },
                    comment: "The only parameter".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_parse_modifier_no_param() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_modifier(
            "modifierWithoutParam",
            Some(Parent::Contract("ParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![NatSpecItem {
                kind: NatSpecKind::Notice,
                comment: "The description of the modifier".to_string()
            },]
        );
    }

    #[test]
    fn test_parse_private_function() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "_viewPrivate",
            Some(Parent::Contract("ParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Some private stuff".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "Dev comment for the private function".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_paramName".to_string()
                    },
                    comment: "The parameter name".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return {
                        name: Some("_returned".to_string())
                    },
                    comment: "The returned value".to_string()
                }
            ]
        );
    }

    #[test]
    fn test_parse_multiline_descriptions() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "_viewMultiline",
            Some(Parent::Contract("ParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Some internal stuff".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Separate line".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Third one".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_parse_multiple_same_tag() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "_viewDuplicateTag",
            Some(Parent::Contract("ParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Some internal stuff".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Separate line".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_parse_error() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_error(
            "SimpleError",
            Some(Parent::Interface("IParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![NatSpecItem {
                kind: NatSpecKind::Notice,
                comment: "Thrown whenever something goes wrong".to_string()
            },]
        );
    }

    #[test]
    fn test_parse_event() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_event(
            "SimpleEvent",
            Some(Parent::Interface("IParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![NatSpecItem {
                kind: NatSpecKind::Notice,
                comment: "Emitted whenever something happens".to_string()
            },]
        );
    }

    #[test]
    fn test_parse_struct() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_struct(
            "SimplestStruct",
            Some(Parent::Interface("IParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "A struct holding 2 variables of type uint256".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "a".to_string()
                    },
                    comment: "The first variable".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "b".to_string()
                    },
                    comment: "The second variable".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "This is definitely a struct".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_parse_external_function_no_params() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "viewFunctionNoParams",
            Some(Parent::Interface("IParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "View function with no parameters".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "Natspec for the return value is missing".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return { name: None },
                    comment: "The returned value".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_parse_external_function_params() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "viewFunctionWithParams",
            Some(Parent::Interface("IParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "A function with different style of natspec".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_param1".to_string()
                    },
                    comment: "The first parameter".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_param2".to_string()
                    },
                    comment: "The second parameter".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return { name: None },
                    comment: "The returned value".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_parse_funny_struct() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_struct(
            "SimpleStruct",
            Some(Parent::Contract("ParserTestFunny".to_string())),
            &items,
        );
        assert_eq!(item.natspec, None);
    }

    #[test]
    fn test_parse_funny_variable() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_variable(
            "someVariable",
            Some(Parent::Contract("ParserTestFunny".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Inheritdoc {
                        parent: "IParserTest".to_string()
                    },
                    comment: String::new()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "Providing context".to_string()
                }
            ]
        );
    }

    #[test]
    fn test_parse_funny_constant() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_variable(
            "SOME_CONSTANT",
            Some(Parent::Contract("ParserTestFunny".to_string())),
            &items,
        );
        assert_eq!(item.natspec, None);
    }

    #[test]
    fn test_parse_funny_function_params() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "viewFunctionWithParams",
            Some(Parent::Contract("ParserTestFunny".to_string())),
            &items,
        );
        assert_eq!(item.natspec, None);
    }

    #[test]
    fn test_parse_funny_function_private() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "_viewPrivateMulti",
            Some(Parent::Contract("ParserTestFunny".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Some private stuff".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_paramName".to_string()
                    },
                    comment: "The parameter name".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return {
                        name: Some("_returned".to_string())
                    },
                    comment: "The returned value".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_parse_funny_function_private_single() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "_viewPrivateSingle",
            Some(Parent::Contract("ParserTestFunny".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Some private stuff".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_paramName".to_string()
                    },
                    comment: "The parameter name".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return {
                        name: Some("_returned".to_string())
                    },
                    comment: "The returned value".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_parse_funny_internal() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "_viewInternal",
            Some(Parent::Contract("ParserTestFunny".to_string())),
            &items,
        );
        assert_eq!(item.natspec, None);
    }

    #[test]
    fn test_parse_funny_linter_fail() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "_viewLinterFail",
            Some(Parent::Contract("ParserTestFunny".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Linter fail".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "What have I done".to_string()
                }
            ]
        );
    }

    #[test]
    fn test_parse_funny_empty_return() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "functionUnnamedEmptyReturn",
            Some(Parent::Contract("ParserTestFunny".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "fun fact: there are extra spaces after the 1st return".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return { name: None },
                    comment: String::new()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return { name: None },
                    comment: String::new()
                },
            ]
        );
    }

    #[test]
    fn test_parse_solidity_latest() {
        let contents = include_str!("../../test-data/LatestVersion.sol");
        let solidity_version = detect_solidity_version(contents).unwrap();
        let parser = Parser::create(solidity_version).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
    }
}
