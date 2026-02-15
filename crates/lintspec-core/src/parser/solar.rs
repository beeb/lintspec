//! A parser with [`solar_parse`] as the backend
//!
//! This is the default parser for the CLI.
use std::{
    collections::HashMap,
    io,
    ops::ControlFlow,
    path::{Path, PathBuf},
    str::FromStr as _,
    sync::{Arc, Mutex},
};

use solar_parse::{
    Parser,
    ast::{
        ContractKind, DocComments, FunctionKind, Item, ItemContract, ItemKind, ParameterList, Span,
        Spanned, VariableDefinition,
        interface::{
            Session,
            source_map::{FileName, SourceMap},
        },
        visit::Visit,
    },
    interface::{ColorChoice, source_map::SourceFile},
};

use crate::{
    definitions::{
        Attributes, Definition, Identifier, Parent, Visibility, constructor::ConstructorDefinition,
        contract::ContractDefinition, enumeration::EnumDefinition, error::ErrorDefinition,
        event::EventDefinition, function::FunctionDefinition, interface::InterfaceDefinition,
        library::LibraryDefinition, modifier::ModifierDefinition, structure::StructDefinition,
        variable::VariableDeclaration,
    },
    error::{ErrorKind, Result},
    interner::INTERNER,
    natspec::{NatSpec, parse_comment},
    parser::{DocumentId, Parse, ParsedDocument, complete_text_ranges},
    prelude::OrPanic as _,
    textindex::{TextIndex, TextRange},
};

type Documents = Vec<(DocumentId, Arc<SourceFile>)>;

#[derive(Clone)]
pub struct SolarParser {
    sess: Arc<Session>,
    documents: Arc<Mutex<Documents>>,
}

impl Default for SolarParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SolarParser {
    #[must_use]
    pub fn new() -> Self {
        let source_map = SourceMap::empty();
        let sess = Session::builder()
            .source_map(Arc::new(source_map))
            .with_buffer_emitter(ColorChoice::Auto)
            .build();
        Self {
            sess: Arc::new(sess),
            documents: Arc::new(Mutex::new(Vec::default())),
        }
    }
}

/// A parser using the [`solar_parse`] crate
impl Parse for SolarParser {
    fn parse_document(
        &mut self,
        input: impl io::Read,
        path: Option<impl AsRef<Path>>,
        keep_contents: bool,
    ) -> Result<ParsedDocument> {
        fn inner(
            this: &mut SolarParser,
            mut input: impl io::Read,
            path: Option<PathBuf>,
            keep_contents: bool,
        ) -> Result<ParsedDocument> {
            let pathbuf = path.clone().unwrap_or(PathBuf::from("<stdin>"));
            let mut buf = String::new();
            input
                .read_to_string(&mut buf)
                .map_err(|err| ErrorKind::IOError {
                    path: pathbuf.clone(),
                    err,
                })?;
            let source_map = this.sess.source_map();
            let source_file = source_map
                .new_source_file(path.map_or(FileName::Stdin, FileName::Real), buf) // should never fail since the content was read already
                .map_err(|err| ErrorKind::IOError {
                    path: pathbuf.clone(),
                    err,
                })?;

            let mut definitions = this
                .sess
                .enter_sequential(|| -> solar_parse::interface::Result<_> {
                    let arena = solar_parse::ast::Arena::new();

                    let mut parser = Parser::from_source_file(&this.sess, &arena, &source_file);

                    let ast = parser.parse_file().map_err(|err| err.emit())?;
                    let mut visitor = LintspecVisitor::new(&this.sess);

                    let _ = visitor.visit_source_unit(&ast);
                    Ok(visitor.definitions)
                })
                .map_err(|_| {
                    let message = match this.sess.emitted_errors() {
                        Some(Err(diags)) => diags.to_string(),
                        None | Some(Ok(())) => "unknown error".to_string(),
                    };
                    ErrorKind::ParsingError {
                        path: pathbuf,
                        loc: TextIndex::ZERO,
                        message,
                    }
                })?;

            let document_id = DocumentId::new();
            if keep_contents {
                let mut documents = this
                    .documents
                    .lock()
                    .or_panic("mutex should not be poisoned");
                documents.push((document_id, Arc::clone(&source_file)));
            }
            complete_text_ranges(&source_file.src, &mut definitions);
            Ok(ParsedDocument {
                definitions,
                id: document_id,
            })
        }
        inner(
            self,
            input,
            path.map(|p| p.as_ref().to_path_buf()),
            keep_contents,
        )
    }

    fn get_sources(self) -> Result<HashMap<DocumentId, String>> {
        let sess = Arc::try_unwrap(self.sess).map_err(|_| ErrorKind::DanglingParserReferences)?;
        drop(sess);
        Arc::try_unwrap(self.documents)
            .map_err(|_| ErrorKind::DanglingParserReferences)?
            .into_inner()
            .or_panic("mutex should not be poisoned")
            .into_iter()
            .map(|(id, doc)| {
                let source_file =
                    Arc::try_unwrap(doc).map_err(|_| ErrorKind::DanglingParserReferences)?;
                Ok((
                    id,
                    Arc::try_unwrap(source_file.src)
                        .map_err(|_| ErrorKind::DanglingParserReferences)?,
                ))
            })
            .collect::<Result<HashMap<_, _>>>()
    }
}

/// A custom visitor to extract definitions from the [`solar_parse`] AST
///
/// Most of the items are visited using solar's default [`Visit`] implementation.
pub struct LintspecVisitor<'ast> {
    current_parent: Option<Parent>,
    definitions: Vec<Definition>,
    sess: &'ast Session,
}

impl<'ast> LintspecVisitor<'ast> {
    /// Construct a new visitor
    pub fn new(sess: &'ast Session) -> Self {
        Self {
            current_parent: None,
            definitions: Vec::default(),
            sess,
        }
    }

    /// Retrieve the found definitions
    ///
    /// This is empty until the AST has been visited with [`LintspecVisitor::visit_source_unit`].
    ///
    /// Note that the spans for each definition and their corresponding members/params/returns only contain the `utf8`
    /// byte offset but no line/column information. These can be populated with [`complete_text_ranges`].
    #[must_use]
    pub fn definitions(&self) -> &Vec<Definition> {
        &self.definitions
    }

    /// Convert a [`Span`] to a pair of utf8 offsets as a [`TextRange`]
    ///
    /// Only the utf8 offset of the [`TextRange`] is initially populated, the rest being filled via [`complete_text_ranges`] to avoid duplicate work.
    fn span_to_textrange(&self, span: Span) -> TextRange {
        let local_begin = self.sess.source_map().lookup_byte_offset(span.lo());
        let local_end = self.sess.source_map().lookup_byte_offset(span.hi());

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
}

impl<'ast> Visit<'ast> for LintspecVisitor<'ast> {
    type BreakValue = ();

    /// Visit an item and extract definitions from it, using the corresponding trait
    fn visit_item(&mut self, item: &'ast Item<'ast>) -> ControlFlow<Self::BreakValue> {
        match &item.kind {
            ItemKind::Contract(item_contract) => {
                if let Some(def) = item_contract.extract_definition(item, self) {
                    self.definitions.push(def);
                }
                self.visit_item_contract(item_contract)?;
            }
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

impl Extract for &solar_parse::ast::ItemContract<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let name = INTERNER.get_or_intern(self.name.as_str());

        let (natspec, span) = match extract_natspec(&item.docs, visitor, &[]) {
            Ok(extracted) => {
                let end_bases = self
                    .bases
                    .last()
                    .map_or(self.name.span.hi(), |b| b.span().hi());
                let end_specifiers = self
                    .layout
                    .as_ref()
                    .map_or(self.name.span.hi(), |l| l.span.hi());
                let contract_end = end_bases.max(end_specifiers);
                extracted.map_or_else(
                    || {
                        (
                            None,
                            visitor.span_to_textrange(item.span.with_hi(contract_end)),
                        )
                    },
                    |(natspec, doc_span)| {
                        // If there are natspec in a contract, take the whole doc and contract header as span
                        (
                            Some(natspec),
                            visitor.span_to_textrange(doc_span.with_hi(contract_end)),
                        )
                    },
                )
            }
            Err(e) => return Some(Definition::NatspecParsingError(e.into_inner())),
        };

        Some(match self.kind {
            ContractKind::Contract | ContractKind::AbstractContract => ContractDefinition {
                name,
                span,
                natspec,
            }
            .into(),
            ContractKind::Interface => InterfaceDefinition {
                name,
                span,
                natspec,
            }
            .into(),
            ContractKind::Library => LibraryDefinition {
                name,
                span,
                natspec,
            }
            .into(),
        })
    }
}

impl Extract for &solar_parse::ast::ItemFunction<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let params = variable_definitions_to_identifiers(Some(&self.header.parameters), visitor);

        let returns = variable_definitions_to_identifiers(self.header.returns.as_ref(), visitor);
        let (natspec, span) = match extract_natspec(&item.docs, visitor, &returns) {
            Ok(extracted) => extracted.map_or_else(
                || (None, visitor.span_to_textrange(self.header.span)),
                |(natspec, doc_span)| {
                    // If there are natspec in a fn, take the whole doc and fn header as span
                    (
                        Some(natspec),
                        visitor.span_to_textrange(doc_span.with_hi(self.header.span.hi())),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e.into_inner())),
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
                    name: INTERNER.get_or_intern(
                        self.header.name.as_ref().map_or("modifier", |n| n.as_str()),
                    ),
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
                    name: INTERNER.get_or_intern(
                        self.header.name.as_ref().map_or("function", |n| n.as_str()),
                    ),
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
        let (natspec, span) = match extract_natspec(&item.docs, visitor, &[]) {
            Ok(extracted) => extracted.map_or_else(
                || (None, visitor.span_to_textrange(item.span)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        visitor.span_to_textrange(doc_span.with_hi(item.span.hi())),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e.into_inner())),
        };

        let attributes = Attributes {
            visibility: self.visibility.into(),
            r#override: self.override_.is_some(),
        };

        Some(
            VariableDeclaration {
                parent: visitor.current_parent.clone(),
                name: INTERNER.get_or_intern(self.name.as_ref().map_or("variable", |n| n.as_str())),
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
        let name = INTERNER.get_or_intern(self.name.as_str());

        let members = self
            .fields
            .iter()
            .map(|m| Identifier {
                name: Some(
                    INTERNER.get_or_intern(m.name.as_ref().map_or("member", |n| n.as_str())),
                ),
                span: visitor.span_to_textrange(m.span),
            })
            .collect();

        let (natspec, span) = match extract_natspec(&item.docs, visitor, &[]) {
            Ok(extracted) => extracted.map_or_else(
                || (None, visitor.span_to_textrange(item.span)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        visitor.span_to_textrange(doc_span.with_hi(item.span.hi())),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e.into_inner())),
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
                name: Some(INTERNER.get_or_intern(v.name.as_str())),
                span: visitor.span_to_textrange(v.span),
            })
            .collect();

        let (natspec, span) = match extract_natspec(&item.docs, visitor, &[]) {
            Ok(extracted) => extracted.map_or_else(
                || (None, visitor.span_to_textrange(item.span)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        visitor.span_to_textrange(doc_span.with_hi(item.span.hi())),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e.into_inner())),
        };

        Some(
            EnumDefinition {
                parent: visitor.current_parent.clone(),
                name: INTERNER.get_or_intern(self.name.as_str()),
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
        let params = variable_definitions_to_identifiers(Some(&self.parameters), visitor);

        let (natspec, span) = match extract_natspec(&item.docs, visitor, &[]) {
            Ok(extracted) => extracted.map_or_else(
                || (None, visitor.span_to_textrange(item.span)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        visitor.span_to_textrange(doc_span.with_hi(item.span.hi())),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e.into_inner())),
        };

        Some(
            ErrorDefinition {
                parent: visitor.current_parent.clone(),
                span,
                name: INTERNER.get_or_intern(self.name.as_str()),
                params,
                natspec,
            }
            .into(),
        )
    }
}

impl Extract for &solar_parse::ast::ItemEvent<'_> {
    fn extract_definition(self, item: &Item, visitor: &mut LintspecVisitor) -> Option<Definition> {
        let params = variable_definitions_to_identifiers(Some(&self.parameters), visitor);

        let (natspec, span) = match extract_natspec(&item.docs, visitor, &[]) {
            Ok(extracted) => extracted.map_or_else(
                || (None, visitor.span_to_textrange(item.span)),
                |(natspec, doc_span)| {
                    (
                        Some(natspec),
                        visitor.span_to_textrange(doc_span.with_hi(item.span.hi())),
                    )
                },
            ),
            Err(e) => return Some(Definition::NatspecParsingError(e.into_inner())),
        };

        Some(
            EventDefinition {
                parent: visitor.current_parent.clone(),
                name: INTERNER.get_or_intern(self.name.as_str()),
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
        let name = INTERNER
            .get_or_intern(contract.name.as_str())
            .resolve_with(&INTERNER);
        match contract.kind {
            ContractKind::Contract | ContractKind::AbstractContract => Parent::Contract(name),
            ContractKind::Library => Parent::Library(name),
            ContractKind::Interface => Parent::Interface(name),
        }
    }
}

/// Convert a spanned [`ast::Visibility`][solar_parse::ast::Visibility] into to the corresponding [`Visibility`] type
impl From<Option<Spanned<solar_parse::ast::Visibility>>> for Visibility {
    fn from(visibility: Option<Spanned<solar_parse::ast::Visibility>>) -> Self {
        visibility.as_deref().into()
    }
}

/// Convert solar's [`ast::Visibility`][solar_parse::ast::Visibility] into to the corresponding [`Visibility`] type
impl From<Option<solar_parse::ast::Visibility>> for Visibility {
    fn from(visibility: Option<solar_parse::ast::Visibility>) -> Self {
        visibility.as_ref().into()
    }
}

/// Convert a reference to [`ast::Visibility`][solar_parse::ast::Visibility] into to the corresponding [`Visibility`] type
impl From<Option<&solar_parse::ast::Visibility>> for Visibility {
    fn from(visibility: Option<&solar_parse::ast::Visibility>) -> Self {
        match visibility {
            Some(solar_parse::ast::Visibility::Public) => Visibility::Public,
            Some(solar_parse::ast::Visibility::Private) => Visibility::Private,
            Some(solar_parse::ast::Visibility::External) => Visibility::External,
            Some(solar_parse::ast::Visibility::Internal) | None => Visibility::Internal,
        }
    }
}

/// Convert a list of [`VariableDefinition`] (used for fn params or returns) into an [`Identifier`]
fn variable_definitions_to_identifiers(
    variable_definitions: Option<&ParameterList>,
    visitor: &mut LintspecVisitor,
) -> Vec<Identifier> {
    let Some(variable_definitions) = variable_definitions else {
        return Vec::new();
    };
    variable_definitions
        .iter()
        .map(|r: &VariableDefinition<'_>| {
            // If there is a named variable, we use its span
            if let Some(name) = r.name {
                Identifier {
                    name: Some(INTERNER.get_or_intern(name.as_str())),
                    span: visitor.span_to_textrange(name.span),
                }
                // Otherwise, we use the return span
            } else {
                Identifier {
                    name: None,
                    span: visitor.span_to_textrange(r.span),
                }
            }
        })
        .collect()
}

/// Convert solar's [`DocComments`] into a [`NatSpec`] and [`Span`]
fn extract_natspec(
    docs: &DocComments,
    visitor: &mut LintspecVisitor,
    returns: &[Identifier],
) -> Result<Option<(NatSpec, Span)>> {
    if docs.is_empty() {
        return Ok(None);
    }
    let mut combined = NatSpec::default();

    for doc in docs.iter() {
        let snippet = visitor
            .sess
            .source_map()
            .span_to_snippet(doc.span)
            .map_err(|e| {
                // there should only be one file in the source map
                let path = visitor.sess.source_map().files().first().map_or(
                    PathBuf::from("<stdin>"),
                    |f| {
                        PathBuf::from_str(&f.name.display().to_string())
                            .unwrap_or(PathBuf::from("<unsupported path>"))
                    },
                );
                ErrorKind::ParsingError {
                    path,
                    loc: visitor.span_to_textrange(doc.span).start,
                    message: format!("{e:?}"),
                }
            })?;

        let mut parsed = parse_comment(&mut snippet.as_str())
            .map_err(|e| ErrorKind::NatspecParsingError {
                parent: visitor.current_parent.clone(),
                span: visitor.span_to_textrange(doc.span),
                message: e.to_string(),
            })?
            .populate_returns(returns);
        combined.append(&mut parsed);
    }

    Ok(Some((combined, docs.span())))
}
