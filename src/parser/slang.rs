//! A parser with `[slang_solidity]` backend
use std::{
    io,
    path::{Path, PathBuf},
};

use slang_solidity::{
    cst::{
        Cursor, NonterminalKind, Query, QueryMatch, TerminalKind, TextIndex as SlangTextIndex,
        TextRange as SlangTextRange,
    },
    parser::Parser,
};

use crate::{
    definitions::{
        constructor::ConstructorDefinition, enumeration::EnumDefinition, error::ErrorDefinition,
        event::EventDefinition, function::FunctionDefinition, modifier::ModifierDefinition,
        structure::StructDefinition, variable::VariableDeclaration, Attributes, Definition,
        Identifier, Parent, TextIndex, TextRange, Visibility,
    },
    error::{Error, Result},
    natspec::{parse_comment, NatSpec},
    utils::{detect_solidity_version, get_latest_supported_version},
};

use super::{Parse, ParsedDocument};

/// A parser that uses [`slang_solidity`] to identify source items
#[derive(Debug, Clone, Default, bon::Builder)]
#[non_exhaustive]
pub struct SlangParser {
    #[builder(default)]
    pub skip_version_detection: bool,
}

impl SlangParser {
    /// The `slang` queries for all source items
    #[must_use]
    pub fn queries() -> Vec<Query> {
        vec![
            ConstructorDefinition::query(),
            EnumDefinition::query(),
            ErrorDefinition::query(),
            EventDefinition::query(),
            FunctionDefinition::query(),
            ModifierDefinition::query(),
            StructDefinition::query(),
            VariableDeclaration::query(),
        ]
    }

    /// Find source item definitions from a root CST [`Cursor`]
    pub fn find_items(cursor: Cursor) -> Vec<Definition> {
        let mut out = Vec::new();
        for m in cursor.query(Self::queries()) {
            let def = match m.query_number {
                0 => Some(
                    ConstructorDefinition::extract(m)
                        .unwrap_or_else(Definition::NatspecParsingError),
                ),
                1 => {
                    Some(EnumDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError))
                }
                2 => Some(
                    ErrorDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError),
                ),
                3 => Some(
                    EventDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError),
                ),
                4 => {
                    let def = FunctionDefinition::extract(m)
                        .unwrap_or_else(Definition::NatspecParsingError);
                    if out.contains(&def) {
                        None
                    } else {
                        Some(def)
                    }
                }
                5 => {
                    let def = ModifierDefinition::extract(m)
                        .unwrap_or_else(Definition::NatspecParsingError);
                    if out.contains(&def) {
                        None
                    } else {
                        Some(def)
                    }
                }
                6 => Some(
                    StructDefinition::extract(m).unwrap_or_else(Definition::NatspecParsingError),
                ),
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
}

impl Parse for SlangParser {
    fn parse_document(
        &mut self,
        mut input: impl io::Read,
        path: Option<impl AsRef<Path>>,
        keep_contents: bool,
    ) -> Result<ParsedDocument> {
        let path = path.map_or(PathBuf::from("<stdin>"), |p| p.as_ref().to_path_buf());
        let (contents, output) = {
            let mut contents = String::new();
            input
                .read_to_string(&mut contents)
                .map_err(|err| Error::IOError {
                    path: PathBuf::new(),
                    err,
                })?;
            let solidity_version = if self.skip_version_detection {
                get_latest_supported_version()
            } else {
                detect_solidity_version(&contents, &path)?
            };
            let parser = Parser::create(solidity_version).expect("parser should initialize");
            let output = parser.parse(NonterminalKind::SourceUnit, &contents);
            (keep_contents.then_some(contents), output)
        };
        if !output.is_valid() {
            let Some(error) = output.errors().first() else {
                return Err(Error::UnknownError);
            };
            return Err(Error::ParsingError {
                path,
                loc: error.text_range().start.into(),
                message: error.message(),
            });
        }
        let cursor = output.create_tree_cursor();
        Ok(ParsedDocument {
            definitions: Self::find_items(cursor),
            contents,
        })
    }
}

/// A trait to extract definitions from a [`slang_solidity`] CST
pub trait Extract {
    /// Return a [`slang_solidity`] [`Query`] used to extract information about the source item
    fn query() -> Query;

    /// Extract information from the query matches
    fn extract(m: QueryMatch) -> Result<Definition>;
}

impl Extract for ConstructorDefinition {
    fn query() -> Query {
        Query::parse(
            "@constructor [ConstructorDefinition
            parameters:[ParametersDeclaration
                @constructor_params parameters:[Parameters]
            ]
            @constructor_attr attributes:[ConstructorAttributes]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let constructor = capture(&m, "constructor")?;
        let params = capture(&m, "constructor_params")?;
        let attr = capture(&m, "constructor_attr")?;

        let span_start = find_definition_start(&constructor);
        let span_end = attr.text_range().end.into();
        let span = span_start..span_end;

        let params = extract_params(&params, NonterminalKind::Parameter);
        let natspec = extract_comment(&constructor.clone(), &[])?;
        let parent = extract_parent_name(constructor);

        Ok(ConstructorDefinition {
            parent,
            span,
            params,
            natspec,
        }
        .into())
    }
}

impl Extract for EnumDefinition {
    fn query() -> Query {
        Query::parse(
            "@enum [EnumDefinition
            @enum_name name:[Identifier]
            @enum_members members:[EnumMembers]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let enumeration = capture(&m, "enum")?;
        let name = capture(&m, "enum_name")?;
        let members = capture(&m, "enum_members")?;

        let span = find_definition_start(&enumeration)..find_definition_end(&enumeration);
        let name = name.node().unparse().trim().to_string();
        let members = extract_enum_members(&members);
        let natspec = extract_comment(&enumeration.clone(), &[])?;
        let parent = extract_parent_name(enumeration);

        Ok(EnumDefinition {
            parent,
            name,
            span,
            members,
            natspec,
        }
        .into())
    }
}

impl Extract for ErrorDefinition {
    fn query() -> Query {
        Query::parse(
            "@err [ErrorDefinition
            @err_name name:[Identifier]
            @err_params members:[ErrorParametersDeclaration]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let err = capture(&m, "err")?;
        let name = capture(&m, "err_name")?;
        let params = capture(&m, "err_params")?;

        let span = find_definition_start(&err)..find_definition_end(&err);
        let name = name.node().unparse().trim().to_string();
        let params = extract_identifiers(&params);
        let natspec = extract_comment(&err.clone(), &[])?;
        let parent = extract_parent_name(err);

        Ok(ErrorDefinition {
            parent,
            name,
            span,
            params,
            natspec,
        }
        .into())
    }
}

impl Extract for EventDefinition {
    fn query() -> Query {
        Query::parse(
            "@event [EventDefinition
            @event_name name:[Identifier]
            @event_params parameters:[EventParametersDeclaration]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let event = capture(&m, "event")?;
        let name = capture(&m, "event_name")?;
        let params = capture(&m, "event_params")?;

        let span = find_definition_start(&event)..find_definition_end(&event);
        let name = name.node().unparse().trim().to_string();
        let params = extract_params(&params, NonterminalKind::EventParameter);
        let natspec = extract_comment(&event.clone(), &[])?;
        let parent = extract_parent_name(event);

        Ok(EventDefinition {
            parent,
            name,
            span,
            params,
            natspec,
        }
        .into())
    }
}

impl Extract for FunctionDefinition {
    fn query() -> Query {
        Query::parse(
            "@function [FunctionDefinition
            @keyword function_keyword:[FunctionKeyword]
            @function_name name:[FunctionName]
            parameters:[ParametersDeclaration
                @function_params parameters:[Parameters]
            ]
            @function_attr attributes:[FunctionAttributes]
            returns:[ReturnsDeclaration
                @function_returns_declaration variables:[ParametersDeclaration
                    @function_returns parameters:[Parameters]
                ]
            ]?
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let func = capture(&m, "function")?;
        let name = capture(&m, "function_name")?;
        let params = capture(&m, "function_params")?;
        let attributes = capture(&m, "function_attr")?;
        let returns_declaration = capture_opt(&m, "function_returns_declaration")?;
        let returns = capture_opt(&m, "function_returns")?;

        let span_start = find_definition_start(&func);
        let span_end = returns_declaration
            .as_ref()
            .map_or_else(|| attributes.text_range().end.into(), find_definition_end);
        let span = span_start..span_end;
        let name = name.node().unparse().trim().to_string();
        let params = extract_params(&params, NonterminalKind::Parameter);
        let returns = returns
            .map(|r| extract_params(&r, NonterminalKind::Parameter))
            .unwrap_or_default();
        let natspec = extract_comment(&func.clone(), &returns)?;
        let parent = extract_parent_name(func);

        Ok(FunctionDefinition {
            parent,
            name,
            span,
            params,
            returns,
            natspec,
            attributes: extract_attributes(&attributes),
        }
        .into())
    }
}

impl Extract for ModifierDefinition {
    fn query() -> Query {
        Query::parse(
            "@modifier [ModifierDefinition
            @modifier_name name:[Identifier]
            parameters:[ParametersDeclaration
                @modifier_params parameters:[Parameters]
            ]?
            @modifier_attr attributes:[ModifierAttributes]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let modifier = capture(&m, "modifier")?;
        let name = capture(&m, "modifier_name")?;
        let params = capture_opt(&m, "modifier_params")?;
        let attr = capture(&m, "modifier_attr")?;

        let span_start = find_definition_start(&modifier);
        let span_end = attr.text_range().end.into();
        let span = span_start..span_end;
        let name = name.node().unparse().trim().to_string();
        let params = params
            .map(|p| extract_params(&p, NonterminalKind::Parameter))
            .unwrap_or_default();

        let natspec = extract_comment(&modifier.clone(), &[])?;
        let parent = extract_parent_name(modifier);

        Ok(ModifierDefinition {
            parent,
            name,
            span,
            params,
            natspec,
            attributes: extract_attributes(&attr),
        }
        .into())
    }
}

impl Extract for StructDefinition {
    fn query() -> Query {
        Query::parse(
            "@struct [StructDefinition
            @struct_name name:[Identifier]
            @struct_members members:[StructMembers]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let structure = capture(&m, "struct")?;
        let name = capture(&m, "struct_name")?;
        let members = capture(&m, "struct_members")?;

        let span = find_definition_start(&structure)..find_definition_end(&structure);
        let name = name.node().unparse().trim().to_string();
        let members = extract_struct_members(&members)?;
        let natspec = extract_comment(&structure.clone(), &[])?;
        let parent = extract_parent_name(structure);

        Ok(StructDefinition {
            parent,
            name,
            span,
            members,
            natspec,
        }
        .into())
    }
}

impl Extract for VariableDeclaration {
    fn query() -> Query {
        Query::parse(
            "@variable [StateVariableDefinition
            @variable_attr attributes:[StateVariableAttributes]
            @variable_name name:[Identifier]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let variable = capture(&m, "variable")?;
        let attributes = capture(&m, "variable_attr")?;
        let name = capture(&m, "variable_name")?;

        let span = find_definition_start(&variable)..find_definition_end(&variable);
        let name = name.node().unparse().trim().to_string();
        let natspec = extract_comment(&variable.clone(), &[])?;
        let parent = extract_parent_name(variable);

        Ok(VariableDeclaration {
            parent,
            name,
            span,
            natspec,
            attributes: extract_attributes(&attributes),
        }
        .into())
    }
}

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
                span: textrange(sub_cursor.text_range()),
            });
        }
        if !found {
            out.push(Identifier {
                name: None,
                span: find_definition_start(&cursor)..find_definition_end(&cursor),
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
            let trimmed = comment.trim_start();
            if trimmed.starts_with("////") || trimmed.starts_with("/***") {
                // avoid a parsing error in those cases, we simply ignore those as if they were non-NatSpec comments
                continue;
            }
            items.push((
                cursor.node().kind().to_string(), // the node type to differentiate multiline from single line
                cursor.text_range().start.line, // the line number to remove unwanted single-line comments
                parse_comment(&mut comment.as_str())
                    .map_err(|e| Error::NatspecParsingError {
                        parent: extract_parent_name(cursor.clone()),
                        span: textrange(cursor.text_range()),
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
            span: textrange(cursor.text_range()),
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

/// Extract the identifiers of each of an enum's variants
#[must_use]
pub fn extract_enum_members(cursor: &Cursor) -> Vec<Identifier> {
    let mut cursor = cursor.spawn().with_edges();
    let mut out = Vec::new();
    while cursor.go_to_next_terminal_with_kind(TerminalKind::Identifier) {
        out.push(Identifier {
            name: Some(cursor.node().unparse().trim().to_string()),
            span: textrange(cursor.text_range()),
        });
    }
    out
}

/// Extract the identifiers for each of a struct's members
pub fn extract_struct_members(cursor: &Cursor) -> Result<Vec<Identifier>> {
    let cursor = cursor.spawn();
    let mut out = Vec::new();
    let query = Query::parse(
        "[StructMember
        @member_name name:[Identifier]
    ]",
    )
    .expect("query should compile");
    for m in cursor.query(vec![query]) {
        let member_name = capture(&m, "member_name")?;
        out.push(Identifier {
            name: Some(member_name.node().unparse().trim().to_string()),
            span: textrange(member_name.text_range()),
        });
    }
    Ok(out)
}

/// Find the end index of a definition by ignoring the trailing trivia
#[must_use]
pub fn find_definition_end(cursor: &Cursor) -> TextIndex {
    let default = cursor.text_range().end.into();
    let mut cursor = cursor.spawn();
    if !cursor.go_to_last_child() {
        return default;
    }
    if !cursor.node().is_trivia() {
        return cursor.text_range().end.into();
    }
    while cursor.go_to_previous() {
        if cursor.node().is_trivia() {
            continue;
        }
        return cursor.text_range().end.into();
    }
    default
}

/// Find the start index of a definition by ignoring any leading whitespace trivia
#[must_use]
pub fn find_definition_start(cursor: &Cursor) -> TextIndex {
    let default = cursor.text_range().start.into();
    let mut cursor = cursor.spawn();
    while cursor.go_to_next() {
        if cursor.node().is_terminal_with_kinds(&[
            TerminalKind::Whitespace,
            TerminalKind::EndOfLine,
            TerminalKind::SingleLineComment,
            TerminalKind::MultiLineComment,
        ]) {
            continue;
        }
        // special case for state variables, since the doc-comment is inside of the type node for some reason
        if cursor.node().is_nonterminal_with_kinds(&[
            NonterminalKind::TypeName,
            NonterminalKind::ElementaryType,
            NonterminalKind::IdentifierPath, // Some state variables have an EndOfLine inside the type identifier
        ]) {
            continue;
        }
        if cursor.node().is_terminal_with_kinds(&[
            TerminalKind::SingleLineNatSpecComment,
            TerminalKind::MultiLineNatSpecComment,
        ]) {
            let comment = cursor.node().unparse();
            let comment = comment.trim_start();
            if comment.starts_with("////") || comment.starts_with("/***") {
                // those should not be considered valid doc-comments, they are normal comments
                continue;
            }
        }
        return cursor.text_range().start.into();
    }
    default
}

/// Convert from a slang `TextRange` to this crate's equivalent
#[must_use]
pub fn textrange(value: SlangTextRange) -> TextRange {
    value.start.into()..value.end.into()
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

#[cfg(test)]
mod tests {
    use std::fs::File;

    use similar_asserts::assert_eq;
    use slang_solidity::{
        cst::{Cursor, NonterminalKind},
        parser::Parser,
    };

    use crate::{
        natspec::{NatSpecItem, NatSpecKind},
        utils::detect_solidity_version,
    };

    use super::*;

    fn parse_file(contents: &str) -> Cursor {
        let solidity_version = detect_solidity_version(contents, PathBuf::new()).unwrap();
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
        let items = SlangParser::find_items(cursor);
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
                    comment: String::new(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "Dev comment for the function".to_string(),
                    span: TextRange::default()
                }
            ]
        );
    }

    #[test]
    fn test_parse_constant() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                comment: String::new(),
                span: TextRange::default()
            },]
        );
    }

    #[test]
    fn test_parse_variable() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                comment: String::new(),
                span: TextRange::default()
            },]
        );
    }

    #[test]
    fn test_parse_modifier() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                    comment: "The description of the modifier".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_param1".to_string()
                    },
                    comment: "The only parameter".to_string(),
                    span: TextRange::default()
                },
            ]
        );
    }

    #[test]
    fn test_parse_modifier_no_param() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
        let item = find_modifier(
            "modifierWithoutParam",
            Some(Parent::Contract("ParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![NatSpecItem {
                kind: NatSpecKind::Notice,
                comment: "The description of the modifier".to_string(),
                span: TextRange::default()
            },]
        );
    }

    #[test]
    fn test_parse_private_function() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                    comment: "Some private stuff".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "Dev comment for the private function".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_paramName".to_string()
                    },
                    comment: "The parameter name".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return {
                        name: Some("_returned".to_string())
                    },
                    comment: "The returned value".to_string(),
                    span: TextRange::default()
                }
            ]
        );
    }

    #[test]
    fn test_parse_multiline_descriptions() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                    comment: "Some internal stuff".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Separate line".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Third one".to_string(),
                    span: TextRange::default()
                },
            ]
        );
    }

    #[test]
    fn test_parse_multiple_same_tag() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                    comment: "Some internal stuff".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Separate line".to_string(),
                    span: TextRange::default()
                },
            ]
        );
    }

    #[test]
    fn test_parse_error() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
        let item = find_error(
            "SimpleError",
            Some(Parent::Interface("IParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![NatSpecItem {
                kind: NatSpecKind::Notice,
                comment: "Thrown whenever something goes wrong".to_string(),
                span: TextRange::default()
            },]
        );
    }

    #[test]
    fn test_parse_event() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
        let item = find_event(
            "SimpleEvent",
            Some(Parent::Interface("IParserTest".to_string())),
            &items,
        );
        assert_eq!(
            item.natspec.as_ref().unwrap().items,
            vec![NatSpecItem {
                kind: NatSpecKind::Notice,
                comment: "Emitted whenever something happens".to_string(),
                span: TextRange::default()
            },]
        );
    }

    #[test]
    fn test_parse_struct() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                    comment: "A struct holding 2 variables of type uint256".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "a".to_string()
                    },
                    comment: "The first variable".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "b".to_string()
                    },
                    comment: "The second variable".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "This is definitely a struct".to_string(),
                    span: TextRange::default()
                },
            ]
        );
    }

    #[test]
    fn test_parse_external_function_no_params() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                    comment: "View function with no parameters".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "Natspec for the return value is missing".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return { name: None },
                    comment: "The returned value".to_string(),
                    span: TextRange::default()
                },
            ]
        );
    }

    #[test]
    fn test_parse_external_function_params() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                    comment: "A function with different style of natspec".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_param1".to_string()
                    },
                    comment: "The first parameter".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_param2".to_string()
                    },
                    comment: "The second parameter".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return { name: None },
                    comment: "The returned value".to_string(),
                    span: TextRange::default()
                },
            ]
        );
    }

    #[test]
    fn test_parse_funny_struct() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
        let items = SlangParser::find_items(cursor);
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
                    comment: String::new(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "Providing context".to_string(),
                    span: TextRange::default()
                }
            ]
        );
    }

    #[test]
    fn test_parse_funny_constant() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
        let items = SlangParser::find_items(cursor);
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
        let items = SlangParser::find_items(cursor);
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
                    comment: "Some private stuff".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_paramName".to_string()
                    },
                    comment: "The parameter name".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return {
                        name: Some("_returned".to_string())
                    },
                    comment: "The returned value".to_string(),
                    span: TextRange::default()
                },
            ]
        );
    }

    #[test]
    fn test_parse_funny_function_private_single() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                    comment: "Some private stuff".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Param {
                        name: "_paramName".to_string()
                    },
                    comment: "The parameter name".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return {
                        name: Some("_returned".to_string())
                    },
                    comment: "The returned value".to_string(),
                    span: TextRange::default()
                },
            ]
        );
    }

    #[test]
    fn test_parse_funny_internal() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
        let items = SlangParser::find_items(cursor);
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
                    comment: "Linter fail".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Dev,
                    comment: "What have I done".to_string(),
                    span: TextRange::default()
                }
            ]
        );
    }

    #[test]
    fn test_parse_funny_empty_return() {
        let cursor = parse_file(include_str!("../../test-data/ParserTest.sol"));
        let items = SlangParser::find_items(cursor);
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
                    comment: "fun fact: there are extra spaces after the 1st return".to_string(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return { name: None },
                    comment: String::new(),
                    span: TextRange::default()
                },
                NatSpecItem {
                    kind: NatSpecKind::Return { name: None },
                    comment: String::new(),
                    span: TextRange::default()
                },
            ]
        );
    }

    #[test]
    fn test_parse_solidity_latest() {
        let contents = include_str!("../../test-data/LatestVersion.sol");
        let solidity_version = detect_solidity_version(contents, PathBuf::new()).unwrap();
        let parser = Parser::create(solidity_version).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
    }

    #[test]
    fn test_parse_solidity_unsupported() {
        let mut parser = SlangParser::builder().skip_version_detection(true).build();
        let file = File::open("test-data/UnsupportedVersion.sol").unwrap();
        let output = parser.parse_document(file, None::<PathBuf>, false);
        assert!(output.is_ok(), "{output:?}");
    }
}
