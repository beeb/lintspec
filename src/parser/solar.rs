//! A parser with [`solar_parse`] backend
use solar_parse::{
    ast::{
        interface::{
            source_map::{FileName, SourceMap},
            Session,
        },
        visit::Visit,
        ContractKind, DocComments, Item, ItemContract, ItemKind, Span, VariableDefinition,
    },
    interface::ColorChoice,
    Parser,
};
use std::{
    io,
    ops::ControlFlow,
    path::{Path, PathBuf},
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
        let source_map = SourceMap::empty();
        let mut buf = String::new();
        input
            .read_to_string(&mut buf)
            .map_err(|err| Error::IOError {
                path: PathBuf::default(),
                err,
            })?;
        let source_file = source_map
            .new_source_file(
                path.as_ref()
                    .map(|p| FileName::Real(p.as_ref().to_path_buf()))
                    .unwrap_or(FileName::Stdin),
                buf,
            ) // should never fail since the content was read already
            .map_err(|err| Error::IOError {
                path: PathBuf::default(),
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
                path: path
                    .map(|p| p.as_ref().to_path_buf())
                    .unwrap_or(PathBuf::from("stdin")),
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
            definitions,
            // retrieve the original source code if needed, without cloning
            contents: keep_contents.then_some(
                Arc::into_inner(source_file.src)
                    .expect("all Arc references should have been dropped"),
            ),
        })
    }
}

/// A custom visitor to extract definitions from the [`solar_parse`] AST
///
/// Most of the items are visited using solar's default [`Visit`] implementation.
pub struct LintspecVisitor<'ast> {
    current_parent: Vec<Parent>,
    definitions: Vec<Definition>,
    source_map: &'ast SourceMap,
}

impl<'a> LintspecVisitor<'a> {
    pub fn new(source_map: &'a SourceMap) -> Self {
        Self {
            current_parent: Vec::default(),
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
            ItemKind::Pragma(item) => self.visit_pragma_directive(item)?,
            ItemKind::Import(item) => self.visit_import_directive(item)?,
            ItemKind::Using(item) => self.visit_using_directive(item)?,
            ItemKind::Contract(item) => self.visit_item_contract(item)?,
            ItemKind::Function(item_function) => {
                if let Some(def) = item_function.extract_definition(item, self) {
                    self.definitions.push(def);
                }

                self.visit_item_function(item_function)?;
            }
            ItemKind::Variable(var_def) => {
                if let Some(def) = var_def.extract_definition(item, self) {
                    self.definitions.push(def);
                }

                self.visit_variable_definition(var_def)?;
            }
            ItemKind::Struct(item_struct) => {
                if let Some(def) = item_struct.extract_definition(item, self) {
                    self.definitions.push(def);
                }

                self.visit_item_struct(item_struct)?;
            }
            ItemKind::Enum(item_enum) => {
                if let Some(enum_def) = item_enum.extract_definition(item, self) {
                    self.definitions.push(enum_def);
                }

                self.visit_item_enum(item_enum)?;
            }
            ItemKind::Udvt(item) => self.visit_item_udvt(item)?,
            ItemKind::Error(item_error) => {
                if let Some(def) = item_error.extract_definition(item, self) {
                    self.definitions.push(def);
                }

                self.visit_item_error(item_error)?;
            }
            ItemKind::Event(item_event) => {
                if let Some(def) = item_event.extract_definition(item, self) {
                    self.definitions.push(def);
                }

                self.visit_item_event(item_event)?;
            }
        }

        ControlFlow::Continue(())
    }

    // In order to track the parent, we need to maintain a stack of visited contracts
    fn visit_item_contract(
        &mut self,
        contract: &'ast ItemContract<'ast>,
    ) -> ControlFlow<Self::BreakValue> {
        let ItemContract {
            kind: _,
            name,
            bases,
            body,
            ..
        } = contract;

        self.current_parent.push(contract.into());

        self.visit_ident(name)?;
        for base in bases.iter() {
            self.visit_modifier(base)?;
        }
        for item in body.iter() {
            self.visit_item(item)?;
        }

        self.current_parent.pop();

        ControlFlow::Continue(())
    }
}

/// Extract each "component" (parent, params, span, etc) then build a [`Definition`] from it
trait Extract {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition>;
}

impl Extract for &solar_parse::ast::ItemFunction<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let parent = visitor.current_parent.last().cloned();
        let params =
            variable_definitions_to_identifiers(self.header.parameters, visitor.source_map);

        let extracted_natspec =
            extract_natspec(&item.docs, visitor.source_map, parent.as_ref()).unwrap_or(None);
        let returns = variable_definitions_to_identifiers(self.header.returns, visitor.source_map);

        let span = if let Some((_, span)) = extracted_natspec {
            span_to_text_range(span, visitor.source_map)
        } else {
            span_to_text_range(item.span, visitor.source_map)
        };

        let natspec = extracted_natspec.map(|(ns, _)| ns.populate_returns(&returns));

        let def = if self.kind.is_constructor() {
            Definition::Constructor(ConstructorDefinition {
                parent,
                span,
                params,
                natspec,
            })
        } else if self.kind.is_modifier() {
            Definition::Modifier(ModifierDefinition {
                parent,
                span,
                params,
                natspec,
                name: self.header.name.unwrap().to_string(),
                attributes: Attributes {
                    visibility: self.header.visibility.into(),
                    r#override: self.header.override_.is_some(),
                },
            })
        } else if self.kind.is_function() {
            Definition::Function(FunctionDefinition {
                parent,
                name: self.header.name.unwrap().to_string(),
                returns: returns.clone(),
                attributes: Attributes {
                    visibility: self.header.visibility.into(),
                    r#override: self.header.override_.is_some(),
                },
                span,
                params,
                natspec,
            })
        } else {
            return None;
        };

        Some(def)
    }
}

impl Extract for &solar_parse::ast::VariableDefinition<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let parent = visitor.current_parent.last().cloned();

        let extracted_natspec =
            extract_natspec(&item.docs, visitor.source_map, parent.as_ref()).unwrap_or(None);

        let span = if let Some((_, span)) = extracted_natspec {
            span_to_text_range(span, visitor.source_map)
        } else {
            span_to_text_range(item.span, visitor.source_map)
        };

        let natspec = if let Some((natspec, _)) = extracted_natspec {
            Some(natspec)
        } else {
            None
        };

        let attributes = Attributes {
            visibility: self.visibility.into(),
            r#override: self.override_.is_some(),
        };

        Some(Definition::Variable(VariableDeclaration {
            parent,
            name: self.name.unwrap().to_string(),
            span,
            natspec,
            attributes,
        }))
    }
}

impl Extract for &solar_parse::ast::ItemStruct<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let parent = visitor.current_parent.last().cloned();

        let name = self.name.to_string();

        let members = self
            .fields
            .iter()
            .map(|m| Identifier {
                name: Some(m.name.expect("name").to_string()),
                span: span_to_text_range(m.span, visitor.source_map),
            })
            .collect();

        let extracted_natspec =
            extract_natspec(&item.docs, visitor.source_map, parent.as_ref()).unwrap_or(None);

        let span = if let Some((_, doc_span)) = &extracted_natspec {
            span_to_text_range(*doc_span, visitor.source_map)
        } else {
            span_to_text_range(item.span, visitor.source_map)
        };

        let natspec = extracted_natspec.map(|(ns, _)| ns);

        Some(Definition::Struct(StructDefinition {
            parent,
            name,
            span,
            members,
            natspec,
        }))
    }
}

impl Extract for &solar_parse::ast::ItemEnum<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let parent = visitor.current_parent.last().cloned();

        let members = self
            .variants
            .iter()
            .map(|v| Identifier {
                name: Some(v.name.to_string()),
                span: span_to_text_range(v.span, visitor.source_map),
            })
            .collect();

        let extracted =
            extract_natspec(&item.docs, visitor.source_map, parent.as_ref()).unwrap_or(None);

        let span = if let Some((_, doc_span)) = &extracted {
            span_to_text_range(*doc_span, visitor.source_map)
        } else {
            span_to_text_range(item.span, visitor.source_map)
        };

        let natspec = extracted.map(|(ns, _)| ns);

        Some(Definition::Enumeration(EnumDefinition {
            parent: parent.clone(),
            name: self.name.to_string(),
            span,
            members,
            natspec,
        }))
    }
}

impl Extract for &solar_parse::ast::ItemError<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let parent = visitor.current_parent.last().cloned();

        let params = variable_definitions_to_identifiers(self.parameters, visitor.source_map);

        let extracted_natspec =
            extract_natspec(&item.docs, visitor.source_map, parent.as_ref()).unwrap_or(None);

        let natspec = extracted_natspec.clone().map(|(ns, _)| ns);

        let span = if let Some((_, span)) = extracted_natspec {
            span_to_text_range(span, visitor.source_map)
        } else {
            span_to_text_range(item.span, visitor.source_map)
        };

        Some(Definition::Error(ErrorDefinition {
            parent: parent.clone(),
            span,
            name: self.name.to_string(),
            params,
            natspec,
        }))
    }
}

impl Extract for &solar_parse::ast::ItemEvent<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let parent = visitor.current_parent.last().cloned();

        let params = variable_definitions_to_identifiers(self.parameters, visitor.source_map);

        let extracted_natspec =
            extract_natspec(&item.docs, visitor.source_map, parent.as_ref()).unwrap_or(None);

        let span = if let Some((_, span)) = extracted_natspec {
            span_to_text_range(span, visitor.source_map)
        } else {
            span_to_text_range(item.span, visitor.source_map)
        };

        let natspec = if let Some((natspec, _)) = extracted_natspec {
            Some(natspec)
        } else {
            None
        };

        Some(Definition::Event(EventDefinition {
            parent: parent.clone(),
            name: self.name.to_string(),
            span,
            params,
            natspec,
        }))
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

/// Convert a [`Span`] to a [`TextRange`]
fn span_to_text_range(span: Span, source_map: &SourceMap) -> TextRange {
    let local_begin = source_map.lookup_byte_offset(span.lo());
    let local_end = source_map.lookup_byte_offset(span.hi());
    let range = local_begin.pos.to_usize()..local_end.pos.to_usize();

    let mut inside = false;
    let mut start = TextIndex::ZERO;
    let mut end = TextIndex::ZERO;
    let mut iter = local_begin.sf.src.chars().peekable();
    while let Some(c) = iter.next() {
        if !inside {
            start.advance(c, iter.peek());
            if start.utf8 >= range.start {
                inside = true;
                end = start;
            }
        } else {
            if end.utf8 >= range.end {
                break;
            }
            end.advance(c, iter.peek());
        }
    }

    start..end
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
                span: span_to_text_range(r.span, source_map),
            }
        })
        .collect()
}

/// Convert solar's [`DocComments`] into a [`NatSpec`] and [`Span`]
fn extract_natspec(
    docs: &DocComments,
    source_map: &SourceMap,
    parent: Option<&Parent>,
) -> Result<Option<(NatSpec, Span)>> {
    if docs.is_empty() {
        return Ok(None);
    }

    let mut combined = NatSpec::default();

    for doc in docs.iter() {
        let snippet = source_map.span_to_snippet(doc.span).map_err(
            |e: solar_parse::interface::source_map::SpanSnippetError| Error::NatspecParsingError {
                parent: parent.cloned(),
                span: span_to_text_range(doc.span, source_map),
                message: format!("{e:?}"),
            },
        )?;

        combined.append(&mut parse_comment(&mut snippet.as_str()).map_err(|e| {
            Error::NatspecParsingError {
                parent: parent.cloned(),
                span: span_to_text_range(doc.span, source_map),
                message: format!("{e:?}"),
            }
        })?);
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
