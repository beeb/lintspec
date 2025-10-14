//! A parser with [`solar_parse`] backend
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
use std::{
    io,
    ops::ControlFlow,
    path::{Path, PathBuf},
    str::FromStr as _,
    sync::{Arc, Mutex},
};

use crate::{
    definitions::{
        Attributes, Definition, Identifier, Parent, TextIndex, TextRange, Visibility,
        constructor::ConstructorDefinition, contract::ContractDefinition,
        enumeration::EnumDefinition, error::ErrorDefinition, event::EventDefinition,
        function::FunctionDefinition, interface::InterfaceDefinition, library::LibraryDefinition,
        modifier::ModifierDefinition, structure::StructDefinition, variable::VariableDeclaration,
    },
    error::{Error, Result},
    natspec::{NatSpec, parse_comment},
    parser::{DocumentId, Parse, ParsedDocument},
};

use std::collections::HashMap;

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
                .map_err(|err| Error::IOError {
                    path: pathbuf.clone(),
                    err,
                })?;
            let source_map = this.sess.source_map();
            let source_file = source_map
                .new_source_file(path.map_or(FileName::Stdin, FileName::Real), buf) // should never fail since the content was read already
                .map_err(|err| Error::IOError {
                    path: pathbuf.clone(),
                    err,
                })?;

            let definitions = this
                .sess
                .enter_sequential(|| -> solar_parse::interface::Result<_> {
                    let arena = solar_parse::ast::Arena::new();

                    let mut parser = Parser::from_source_file(&this.sess, &arena, &source_file);

                    let ast = parser.parse_file().map_err(|err| err.emit())?;
                    let mut visitor = LintspecVisitor::new(source_map);

                    let _ = visitor.visit_source_unit(&ast);
                    Ok(visitor.definitions)
                })
                .map_err(|_| Error::ParsingError {
                    path: pathbuf,
                    loc: TextIndex::ZERO,
                    message: this
                        .sess
                        .emitted_errors()
                        .expect("should have a Result")
                        .unwrap_err()
                        .to_string(),
                })?;

            let document_id = DocumentId::new();
            if keep_contents {
                let mut documents = this.documents.lock().expect("mutex should not be poisoned");
                documents.push((document_id, Arc::clone(&source_file)));
            }
            Ok(ParsedDocument {
                definitions: complete_text_ranges(&source_file.src, definitions),
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
        let sess = Arc::try_unwrap(self.sess).map_err(|_| Error::DanglingParserReferences)?;
        drop(sess);
        Ok(Arc::try_unwrap(self.documents)
            .expect("all references should have been dropped")
            .into_inner()
            .expect("mutex should not be poisoned")
            .into_iter()
            .map(|(id, doc)| {
                let source_file = Arc::try_unwrap(doc)
                    .expect("all SourceMap references should have been dropped");
                (
                    id,
                    Arc::try_unwrap(source_file.src)
                        .expect("all SourceFile references should have been dropped"),
                )
            })
            .collect())
    }
}

#[allow(clippy::too_many_lines)]
/// Complete the [`TextRange`] of a list of [`Definition`]
fn complete_text_ranges(source: &str, mut definitions: Vec<Definition>) -> Vec<Definition> {
    fn register_span(set: &mut Vec<usize>, span: &TextRange) {
        set.push(span.start.utf8);
        set.push(span.end.utf8);
    }
    fn populate_span(map: &[(usize, TextIndex)], start_idx: usize, span: &mut TextRange) -> usize {
        let res;
        (res, span.start) = map
            .iter()
            .enumerate()
            .skip(start_idx)
            .find_map(|(i, (utf8, ti))| (utf8 == &span.start.utf8).then_some((i, *ti)))
            .expect("utf8 start offset should be present in cache");
        span.end = map
            .iter()
            .skip(res + 1)
            .find_map(|(utf8, ti)| (utf8 == &span.end.utf8).then_some(*ti))
            .expect("utf8 end offset should be present in cache");
        res + 1
    }
    let mut offsets = Vec::with_capacity(definitions.len() * 2); // lower bound for the size
    for def in &definitions {
        def.span().inspect(|s| register_span(&mut offsets, s));
        match def {
            Definition::Constructor(d) => {
                d.params
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
            }
            Definition::Enumeration(d) => {
                d.members
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
            }
            Definition::Error(d) => {
                d.params
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
            }
            Definition::Event(d) => {
                d.params
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
            }
            Definition::Function(d) => {
                d.params
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
                d.returns
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
            }
            Definition::Modifier(d) => {
                d.params
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
            }
            Definition::Struct(d) => {
                d.members
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
            }
            Definition::NatspecParsingError(Error::NatspecParsingError { span, .. }) => {
                register_span(&mut offsets, span);
            }
            Definition::Contract(_)
            | Definition::Interface(_)
            | Definition::Library(_)
            | Definition::Variable(_)
            | Definition::NatspecParsingError(_) => {}
        }
    }
    if offsets.is_empty() {
        return definitions;
    }
    offsets.sort_unstable();

    let mut text_indices = Vec::with_capacity(offsets.len()); // upper bound for the size
    let mut current = TextIndex::ZERO;
    text_indices.push((0, current)); // just in case zero is needed

    let mut set_iter = offsets.iter();
    let mut char_iter = source.chars().peekable();
    let mut current_offset = set_iter
        .next()
        .expect("there should be one element at least");
    while let Some(c) = char_iter.next() {
        debug_assert!(current_offset >= &current.utf8);
        current.advance(c, char_iter.peek());
        match current.utf8.cmp(current_offset) {
            std::cmp::Ordering::Equal => {
                text_indices.push((current.utf8, current));
            }
            std::cmp::Ordering::Greater => {
                current_offset = match set_iter.find(|o| o != &current_offset) {
                    Some(o) => o,
                    None => break,
                };
                if current_offset == &current.utf8 {
                    text_indices.push((current.utf8, current));
                }
            }
            std::cmp::Ordering::Less => {}
        }
    }

    let mut idx = 0;
    for def in &mut definitions {
        if let Some(span) = def.span_mut() {
            idx = populate_span(&text_indices, idx, span);
        }
        match def {
            Definition::Constructor(d) => {
                d.params
                    .iter_mut()
                    .for_each(|i| idx = populate_span(&text_indices, idx, &mut i.span));
            }
            Definition::Enumeration(d) => {
                d.members
                    .iter_mut()
                    .for_each(|i| idx = populate_span(&text_indices, idx, &mut i.span));
            }
            Definition::Error(d) => {
                d.params
                    .iter_mut()
                    .for_each(|i| idx = populate_span(&text_indices, idx, &mut i.span));
            }
            Definition::Event(d) => {
                d.params
                    .iter_mut()
                    .for_each(|i| idx = populate_span(&text_indices, idx, &mut i.span));
            }
            Definition::Function(d) => {
                d.params
                    .iter_mut()
                    .for_each(|i| idx = populate_span(&text_indices, idx, &mut i.span));
                d.returns
                    .iter_mut()
                    .for_each(|i| idx = populate_span(&text_indices, idx, &mut i.span));
            }
            Definition::Modifier(d) => {
                d.params
                    .iter_mut()
                    .for_each(|i| idx = populate_span(&text_indices, idx, &mut i.span));
            }
            Definition::Struct(d) => {
                d.members
                    .iter_mut()
                    .for_each(|i| idx = populate_span(&text_indices, idx, &mut i.span));
            }
            Definition::NatspecParsingError(Error::NatspecParsingError { span, .. }) => {
                idx = populate_span(&text_indices, idx, span);
            }
            Definition::Contract(_)
            | Definition::Interface(_)
            | Definition::Library(_)
            | Definition::Variable(_)
            | Definition::NatspecParsingError(_) => {}
        }
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

    /// Convert a [`Span`] to a pair of utf8 offsets as a [`TextRange`]
    ///
    /// Only the utf8 offset of the [`TextRange`] is initially populated, the rest being filled via [`complete_text_ranges`] to avoid duplicate work.
    fn span_to_textrange(&self, span: Span) -> TextRange {
        let local_begin = self.source_map.lookup_byte_offset(span.lo());
        let local_end = self.source_map.lookup_byte_offset(span.hi());

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
        let name = self.name.to_string();

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
            Err(e) => return Some(Definition::NatspecParsingError(e)),
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
                    name: Some(name.to_string()),
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
        let snippet = visitor.source_map.span_to_snippet(doc.span).map_err(|e| {
            // there should only be one file in the source map
            let path = visitor
                .source_map
                .files()
                .first()
                .map_or(PathBuf::from("<stdin>"), |f| {
                    PathBuf::from_str(&f.name.display().to_string())
                        .unwrap_or(PathBuf::from("<unsupported path>"))
                });
            Error::ParsingError {
                path,
                loc: visitor.span_to_textrange(doc.span).start,
                message: format!("{e:?}"),
            }
        })?;

        let mut parsed = parse_comment(&mut snippet.as_str())
            .map_err(|e| Error::NatspecParsingError {
                parent: visitor.current_parent.clone(),
                span: visitor.span_to_textrange(doc.span),
                message: e.to_string(),
            })?
            .populate_returns(returns);
        combined.append(&mut parsed);
    }

    Ok(Some((combined, docs.span())))
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
