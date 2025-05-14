//! A parser with [`solar_parse`] backend
use solar_parse::{
    ast::{
        interface::{
            source_map::{FileName, SourceMap},
            Session,
        },
        visit::Visit,
        ContractKind, DocComments, FunctionKind, Item, ItemContract, ItemKind, Span,
        VariableDefinition,
    },
    interface::ColorChoice,
    Parser,
};
use std::{
    io,
    ops::ControlFlow,
    path::{Path, PathBuf},
    str::FromStr as _,
    sync::Arc,
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
    parser::{Parse, ParsedDocument},
};

use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct SolarParser {}

/// A parser using the [`solar_parse`] crate
impl Parse for SolarParser {
    fn parse_document(
        &mut self,
        mut input: impl io::Read,
        path: Option<impl AsRef<Path>>,
        keep_contents: bool,
    ) -> Result<ParsedDocument> {
        let pathbuf = path
            .as_ref()
            .map_or(PathBuf::from("<stdin>"), |p| p.as_ref().to_path_buf());
        let source_map = SourceMap::empty();
        let mut buf = String::new();
        input
            .read_to_string(&mut buf)
            .map_err(|err| Error::IOError {
                path: pathbuf.clone(),
                err,
            })?;
        let source_file = source_map
            .new_source_file(
                path.as_ref().map_or(FileName::Stdin, |p| {
                    FileName::Real(p.as_ref().to_path_buf())
                }),
                buf,
            ) // should never fail since the content was read already
            .map_err(|err| Error::IOError {
                path: pathbuf.clone(),
                err,
            })?;
        let source_map = Arc::new(source_map);
        let sess = Session::builder()
            .source_map(Arc::clone(&source_map))
            .with_buffer_emitter(ColorChoice::Auto)
            .build();

        let definitions = sess
            .enter(|| -> solar_parse::interface::Result<_> {
                let arena = solar_parse::ast::Arena::new();

                let mut parser = Parser::from_source_file(&sess, &arena, &source_file);

                let ast = parser.parse_file().map_err(|err| err.emit())?;
                let mut visitor = LintspecVisitor::new(&source_map);

                let _ = visitor.visit_source_unit(&ast);
                Ok(visitor.definitions)
            })
            .map_err(|_| Error::ParsingError {
                path: pathbuf.clone(),
                loc: TextIndex::ZERO,
                message: sess
                    .emitted_errors()
                    .expect("should have a Result")
                    .unwrap_err()
                    .to_string(),
            })?;

        drop(source_map);
        drop(sess);
        let source_file =
            Arc::into_inner(source_file).expect("all Arc references should have been dropped");

        Ok(ParsedDocument {
            definitions: complete_text_ranges(&source_file.src, definitions),
            contents: keep_contents.then_some(
                Arc::into_inner(source_file.src)
                    .expect("all Arc references should have been dropped"),
            ),
        })
    }
}

/// Complete the [`TextRange`] of a list of [`Definition`]
fn complete_text_ranges(source: &str, mut definitions: Vec<Definition>) -> Vec<Definition> {
    let utf8_offsets: HashSet<_> = definitions
        .iter()
        .filter_map(|d| {
            if let Some(span) = d.span() {
                Some([span.start.utf8, span.end.utf8])
            } else {
                None
            }
        })
        .flatten()
        .collect();
    let mut mapping = HashMap::new();
    let mut current = TextIndex::ZERO;
    mapping.insert(0, current); // just in case zero is needed

    let mut char_iter = source.chars().peekable();
    while let Some(c) = char_iter.next() {
        current.advance(c, char_iter.peek());

        if utf8_offsets.contains(&current.utf8) {
            mapping.insert(current.utf8, current);
        }
    }
    for span in definitions.iter_mut().filter_map(|d| d.span_mut()) {
        span.start = *mapping
            .get(&span.start.utf8)
            .expect("utf8 offset should be present in cache");
        span.end = *mapping
            .get(&span.end.utf8)
            .expect("utf8 offset should be present in cache");
    }
    definitions
}

/// A custom visitor to extract definitions from the [`solar_parse`] AST
///
/// Most of the items are visited using solar's default [`Visit`] implementation.
pub struct LintspecVisitor<'ast> {
    current_parent: Option<Parent>,
    definitions: Vec<Definition>,
    source_map: &'ast SourceMap,
}

impl<'ast> LintspecVisitor<'ast> {
    pub fn new(source_map: &'ast SourceMap) -> Self {
        Self {
            current_parent: None,
            definitions: Vec::default(),
            source_map,
        }
    }
}

impl<'ast> Visit<'ast> for LintspecVisitor<'ast> {
    type BreakValue = ();

    /// Visit an item and extract definitions from it, using the corresponding trait
    fn visit_item(&mut self, item: &'ast Item<'ast>) -> ControlFlow<Self::BreakValue> {
        match &item.kind {
            ItemKind::Contract(item) => self.visit_item_contract(item)?,
            ItemKind::Function(item_function) => {
                if let Some(def) = item_function.extract_definition(item, self) {
                    self.definitions.push(def);
                }
            }
            ItemKind::Variable(var_def) => {
                if let Some(def) = var_def.extract_definition(item, self) {
                    self.definitions.push(def);
                }
            }
            ItemKind::Struct(item_struct) => {
                if let Some(def) = item_struct.extract_definition(item, self) {
                    self.definitions.push(def);
                }
            }
            ItemKind::Enum(item_enum) => {
                if let Some(enum_def) = item_enum.extract_definition(item, self) {
                    self.definitions.push(enum_def);
                }
            }
            ItemKind::Error(item_error) => {
                if let Some(def) = item_error.extract_definition(item, self) {
                    self.definitions.push(def);
                }
            }
            ItemKind::Event(item_event) => {
                if let Some(def) = item_event.extract_definition(item, self) {
                    self.definitions.push(def);
                }
            }
            ItemKind::Pragma(_) | ItemKind::Import(_) | ItemKind::Using(_) | ItemKind::Udvt(_) => {}
        }

        ControlFlow::Continue(())
    }

    // In order to track the parent, we need to maintain a reference to the contract being visited
    fn visit_item_contract(
        &mut self,
        contract: &'ast ItemContract<'ast>,
    ) -> ControlFlow<Self::BreakValue> {
        let ItemContract { bases, body, .. } = contract;

        self.current_parent = Some(contract.into());

        for base in bases.iter() {
            self.visit_modifier(base)?;
        }
        for item in body.iter() {
            self.visit_item(item)?;
        }

        self.current_parent = None;

        ControlFlow::Continue(())
    }
}

/// Extract each "component" (parent, params, span, etc) then build a [`Definition`] from it
trait Extract {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition>;
}

impl Extract for &solar_parse::ast::ItemFunction<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let params =
            variable_definitions_to_identifiers(self.header.parameters, visitor.source_map);

        let returns = variable_definitions_to_identifiers(self.header.returns, visitor.source_map);

        let (natspec, span) = match extract_natspec(
            &item.docs,
            visitor.source_map,
            visitor.current_parent.as_ref(),
            &returns,
        ) {
            Ok(extracted) => extracted.map_or_else(
                || (None, span_to_utf8_offset(item.span, visitor.source_map)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        span_to_utf8_offset(doc_span, visitor.source_map),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e)),
        };

        match self.kind {
            FunctionKind::Constructor => Some(
                ConstructorDefinition {
                    parent: visitor.current_parent.clone(),
                    span,
                    params,
                    natspec,
                }
                .into(),
            ),
            FunctionKind::Modifier => Some(
                ModifierDefinition {
                    parent: visitor.current_parent.clone(),
                    span,
                    params,
                    natspec,
                    name: self.header.name.unwrap().to_string(),
                    attributes: Attributes {
                        visibility: self.header.visibility.into(),
                        r#override: self.header.override_.is_some(),
                    },
                }
                .into(),
            ),
            FunctionKind::Function => Some(
                FunctionDefinition {
                    parent: visitor.current_parent.clone(),
                    name: self.header.name.unwrap().to_string(),
                    returns: returns.clone(),
                    attributes: Attributes {
                        visibility: self.header.visibility.into(),
                        r#override: self.header.override_.is_some(),
                    },
                    span,
                    params,
                    natspec,
                }
                .into(),
            ),
            FunctionKind::Receive | FunctionKind::Fallback => None,
        }
    }
}

impl Extract for &solar_parse::ast::VariableDefinition<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let (natspec, span) = match extract_natspec(
            &item.docs,
            visitor.source_map,
            visitor.current_parent.as_ref(),
            &[],
        ) {
            Ok(extracted) => extracted.map_or_else(
                || (None, span_to_utf8_offset(item.span, visitor.source_map)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        span_to_utf8_offset(doc_span, visitor.source_map),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e)),
        };

        let attributes = Attributes {
            visibility: self.visibility.into(),
            r#override: self.override_.is_some(),
        };

        Some(
            VariableDeclaration {
                parent: visitor.current_parent.clone(),
                name: self.name.unwrap().to_string(),
                span,
                natspec,
                attributes,
            }
            .into(),
        )
    }
}

impl Extract for &solar_parse::ast::ItemStruct<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let name = self.name.to_string();

        let members = self
            .fields
            .iter()
            .map(|m| Identifier {
                name: Some(m.name.expect("name").to_string()),
                span: span_to_utf8_offset(m.span, visitor.source_map),
            })
            .collect();

        let (natspec, span) = match extract_natspec(
            &item.docs,
            visitor.source_map,
            visitor.current_parent.as_ref(),
            &[],
        ) {
            Ok(extracted) => extracted.map_or_else(
                || (None, span_to_utf8_offset(item.span, visitor.source_map)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        span_to_utf8_offset(doc_span, visitor.source_map),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e)),
        };

        Some(
            StructDefinition {
                parent: visitor.current_parent.clone(),
                name,
                span,
                members,
                natspec,
            }
            .into(),
        )
    }
}

impl Extract for &solar_parse::ast::ItemEnum<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let members = self
            .variants
            .iter()
            .map(|v| Identifier {
                name: Some(v.name.to_string()),
                span: span_to_utf8_offset(v.span, visitor.source_map),
            })
            .collect();

        let (natspec, span) = match extract_natspec(
            &item.docs,
            visitor.source_map,
            visitor.current_parent.as_ref(),
            &[],
        ) {
            Ok(extracted) => extracted.map_or_else(
                || (None, span_to_utf8_offset(item.span, visitor.source_map)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        span_to_utf8_offset(doc_span, visitor.source_map),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e)),
        };

        Some(
            EnumDefinition {
                parent: visitor.current_parent.clone(),
                name: self.name.to_string(),
                span,
                members,
                natspec,
            }
            .into(),
        )
    }
}

impl Extract for &solar_parse::ast::ItemError<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let params = variable_definitions_to_identifiers(self.parameters, visitor.source_map);

        let (natspec, span) = match extract_natspec(
            &item.docs,
            visitor.source_map,
            visitor.current_parent.as_ref(),
            &[],
        ) {
            Ok(extracted) => extracted.map_or_else(
                || (None, span_to_utf8_offset(item.span, visitor.source_map)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        span_to_utf8_offset(doc_span, visitor.source_map),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e)),
        };

        Some(
            ErrorDefinition {
                parent: visitor.current_parent.clone(),
                span,
                name: self.name.to_string(),
                params,
                natspec,
            }
            .into(),
        )
    }
}

impl Extract for &solar_parse::ast::ItemEvent<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let params = variable_definitions_to_identifiers(self.parameters, visitor.source_map);

        let (natspec, span) = match extract_natspec(
            &item.docs,
            visitor.source_map,
            visitor.current_parent.as_ref(),
            &[],
        ) {
            Ok(extracted) => extracted.map_or_else(
                || (None, span_to_utf8_offset(item.span, visitor.source_map)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        span_to_utf8_offset(doc_span, visitor.source_map),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e)),
        };

        Some(
            EventDefinition {
                parent: visitor.current_parent.clone(),
                name: self.name.to_string(),
                span,
                params,
                natspec,
            }
            .into(),
        )
    }
}

/// Create a [`Parent`] from solar's [`ItemContract`]
impl From<&ItemContract<'_>> for Parent {
    fn from(contract: &ItemContract) -> Self {
        match contract.kind {
            ContractKind::Contract | ContractKind::AbstractContract => {
                Parent::Contract(contract.name.to_string())
            }
            ContractKind::Library => Parent::Library(contract.name.to_string()),
            ContractKind::Interface => Parent::Interface(contract.name.to_string()),
        }
    }
}

/// Convert a [`Span`] to an utf8 offset stored in a [`TextRange`]
/// @dev Only the uft8 offset of the TextRante is initially populated, the rest being filled in `complete_text_indices` (avoid multiple src parsing for the utf16)
fn span_to_utf8_offset(span: Span, source_map: &SourceMap) -> TextRange {
    let local_begin = source_map.lookup_byte_offset(span.lo());
    let local_end = source_map.lookup_byte_offset(span.hi());

    let start_utf8 = local_begin.pos.to_usize();
    let end_utf8 = local_end.pos.to_usize();

    let start_index = TextIndex {
        utf8: start_utf8,
        ..Default::default()
    };

    let end_index = TextIndex {
        utf8: end_utf8,
        ..Default::default()
    };

    start_index..end_index
}

/// Convert a list of [`VariableDefinition`] (used for fn params or returns) into an [`Identifier`]
fn variable_definitions_to_identifiers(
    variable_definitions: &[VariableDefinition<'_>],
    source_map: &SourceMap,
) -> Vec<Identifier> {
    variable_definitions
        .iter()
        .map(|r| {
            // Preserve named returns; leave unnamed returns as None
            let name = r.name.map(|n| n.to_string()).filter(|s| !s.is_empty());
            Identifier {
                name,
                span: span_to_utf8_offset(r.span, source_map),
            }
        })
        .collect()
}

/// Convert solar's [`DocComments`] into a [`NatSpec`] and [`Span`]
fn extract_natspec(
    docs: &DocComments,
    source_map: &SourceMap,
    parent: Option<&Parent>,
    returns: &[Identifier],
) -> Result<Option<(NatSpec, Span)>> {
    if docs.is_empty() {
        return Ok(None);
    }
    let mut combined = NatSpec::default();

    for doc in docs.iter() {
        let snippet = source_map.span_to_snippet(doc.span).map_err(|e| {
            // there should only be one file in the source map
            let path = source_map
                .files()
                .first()
                .map_or(PathBuf::from("<stdin>"), |f| {
                    PathBuf::from_str(&f.name.display().to_string())
                        .unwrap_or(PathBuf::from("<unsupported path>"))
                });
            Error::ParsingError {
                path,
                loc: span_to_utf8_offset(doc.span, source_map).start,
                message: format!("{e:?}"),
            }
        })?;

        let mut parsed = parse_comment(&mut snippet.as_str())
            .map_err(|e| Error::NatspecParsingError {
                parent: parent.cloned(),
                span: span_to_utf8_offset(doc.span, source_map),
                message: e.to_string(),
            })?
            .populate_returns(returns);
        combined.append(&mut parsed);
    }

    Ok(Some((combined, docs.span())))
}

/// Convert solar's [`ast::Visibility`][solar_parse::ast::Visibility] into to the corresponding [`Visibility`] type
impl From<Option<solar_parse::ast::Visibility>> for Visibility {
    fn from(visibility: Option<solar_parse::ast::Visibility>) -> Self {
        match visibility {
            Some(solar_parse::ast::Visibility::Public) => Visibility::Public,
            Some(solar_parse::ast::Visibility::Private) => Visibility::Private,
            Some(solar_parse::ast::Visibility::External) => Visibility::External,
            Some(solar_parse::ast::Visibility::Internal) | None => Visibility::Internal,
        }
    }
}
