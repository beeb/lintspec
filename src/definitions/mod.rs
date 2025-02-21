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
pub struct ValidationOptions {
    /// Whether overridden, public and external functions should have an `@inheritdoc`
    pub inheritdoc: bool,

    /// Whether to check that constructors have documented params
    pub constructor: bool,

    /// Whether to check that each member of structs is documented with `@param`
    ///
    /// Not standard practice, the Solidity spec does not consider `@param` for structs or provide any other way to
    /// document each member.
    pub struct_params: bool,

    /// Whether to check that each variant of enums is documented with `@param`
    ///
    /// Not standard practice, the Solidity spec does not consider `@param` for enums or provide any other way to
    /// document each variant.
    pub enum_params: bool,
}

impl Default for ValidationOptions {
    /// Get default validation options (`inheritdoc` is true, the others are false)
    fn default() -> Self {
        Self {
            inheritdoc: true,
            constructor: false,
            struct_params: false,
            enum_params: false,
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
    /// Validate a definition and generate [`Diagnostic`]s for errors
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

    /// Retrieve the inner constructor definition
    pub fn as_constructor(self) -> Option<ConstructorDefinition> {
        match self {
            Definition::Constructor(def) => Some(def),
            _ => None,
        }
    }

    /// Retrieve the inner enum definition
    pub fn as_enum(self) -> Option<EnumDefinition> {
        match self {
            Definition::Enumeration(def) => Some(def),
            _ => None,
        }
    }

    /// Retrieve the inner error definition
    pub fn as_error(self) -> Option<ErrorDefinition> {
        match self {
            Definition::Error(def) => Some(def),
            _ => None,
        }
    }

    /// Retrieve the inner event definition
    pub fn as_event(self) -> Option<EventDefinition> {
        match self {
            Definition::Event(def) => Some(def),
            _ => None,
        }
    }

    /// Retrieve the inner function definition
    pub fn as_function(self) -> Option<FunctionDefinition> {
        match self {
            Definition::Function(def) => Some(def),
            _ => None,
        }
    }

    /// Retrieve the inner modifier definition
    pub fn as_modifier(self) -> Option<ModifierDefinition> {
        match self {
            Definition::Modifier(def) => Some(def),
            _ => None,
        }
    }

    /// Retrieve the inner struct definition
    pub fn as_struct(self) -> Option<StructDefinition> {
        match self {
            Definition::Struct(def) => Some(def),
            _ => None,
        }
    }

    /// Retrieve the inner variable declaration
    pub fn as_variable(self) -> Option<VariableDeclaration> {
        match self {
            Definition::Variable(def) => Some(def),
            _ => None,
        }
    }
}

/// Find source item definitions from a root CST cursor
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

/// Extract parameters from a function-like source item.
///
/// The node kind that holds the `Identifier` (`Parameter`, `EventParameter`) is provided as `kind`.
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

/// Extract and parse the NatSpec comment information, if any
pub fn extract_comment(cursor: Cursor, returns: &[Identifier]) -> Result<Option<NatSpec>> {
    // TODO: should we parse the doc comments inside of structs? filtering them out would add complexity
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
                    parent: extract_parent_name(cursor.clone()),
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

/// Extract identifiers from a CST node, filtered by label equal to `name`
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

/// Check a list of params to see if they are documented with a corresponding item in the NatSpec, and generate a
/// diagnostic for each missing one or if there are more than 1 entry per param.
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

/// Check a list of returns to see if they are documented with a corresponding item in the NatSpec, and generate a
/// diagnostic for each missing one or if there are more than 1 entry per param.
pub fn check_returns(natspec: &NatSpec, returns: &[Identifier]) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    let returns_count = natspec.count_all_returns() as isize;
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
                if idx as isize > returns_count - 1 {
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

/// Extract the attributes (visibility and override) from a function-like item or state variable
pub fn extract_attributes(cursor: Cursor) -> Attributes {
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

#[cfg(test)]
mod tests {
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
                    comment: "".to_string()
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
                comment: "".to_string()
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
                comment: "".to_string()
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
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "The first variable".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "The first variable".to_string()
                }
            ]
        );
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
                    comment: "".to_string()
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

    #[ignore]
    #[test]
    fn test_parse_funny_function_private() {
        // FIXME: should it parse the first comment or nah?
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = find_items(cursor);
        let item = find_function(
            "_viewPrivate",
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
                    comment: "".to_string()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return { name: None },
                    comment: "".to_string()
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
