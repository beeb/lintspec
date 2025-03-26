//! Solidity parser interface
use std::fs;
use std::{num::NonZeroI128, path::Path};

use slang_solidity::cst::TextIndex;
use solar_data_structures::trustme;
use solar_parse::{
    ast::{
        interface::{
            source_map::{FileName, SourceMap},
            Session,
        },
        visit::Visit,
        ContractKind, DocComments, Item, ItemContract, ItemKind, PragmaDirective, SourceUnit, Span,
        VariableDefinition,
    },
    Parser,
};

use slang_solidity::cst::TextRange;
use std::ops::ControlFlow;

use crate::{
    definitions::{
        constructor::ConstructorDefinition, enumeration::EnumDefinition, error::ErrorDefinition,
        event::EventDefinition, function::FunctionDefinition, modifier::ModifierDefinition,
        structure::StructDefinition, variable::VariableDeclaration, Attributes, Definition,
        Identifier, Parent, Visibility,
    },
    error::{Error, Result},
    natspec::{self, parse_comment, NatSpec},
    parser::{Parse, ParsedDocument},
    utils::detect_solidity_version,
};

pub struct SolarParser {}

pub struct LintspecVisitor<'ast> {
    current_parent: Vec<Parent>,
    definitions: Vec<Definition>,
    source_map: &'ast SourceMap,
}

impl Parse for SolarParser {
    fn parse_document(path: impl AsRef<Path>, keep_contents: bool) -> Result<ParsedDocument> {
        let path = path.as_ref().to_path_buf();

        let sess = Session::builder().with_silent_emitter(None).build();
        let source_map = sess.source_map();

        let parsed_document: ParsedDocument = ParsedDocument {
            definitions: Vec::new(),
            contents: None,
        };

        let _ = sess.enter(|| -> solar_parse::interface::Result<()> {
            let arena = solar_parse::ast::Arena::new();

            let mut parser =
                Parser::from_lazy_source_code(&sess, &arena, FileName::from(path.clone()), || {
                    fs::read_to_string(&path)
                })?;

            let ast = parser.parse_file().map_err(|e| e.emit())?;

            let mut visitor = LintspecVisitor {
                current_parent: Vec::new(),
                definitions: Vec::new(),
                source_map,
            };

            let result = visitor.visit_source_unit(&ast);

            // dbg!(&ast);

            Ok(())
        });

        Ok(ParsedDocument {
            definitions: vec![],
            contents: None,
        })
    }
}

impl<'ast> Visit<'ast> for LintspecVisitor<'_> {
    type BreakValue = ();

    fn visit_source_unit(&mut self, source_unit: &SourceUnit) -> ControlFlow<Self::BreakValue> {
        let source_unit = unsafe { trustme::decouple_lt(source_unit) };
        let SourceUnit { items } = source_unit;
        for item in items.iter() {
            self.visit_item(item)?;
        }

        dbg!(&self.definitions);

        ControlFlow::Continue(())
    }

    fn visit_item(&mut self, item: &'ast Item<'ast>) -> ControlFlow<Self::BreakValue> {
        let Item { docs, span, kind } = item;
        self.visit_span(span)?;
        self.visit_doc_comments(docs)?;
        match kind {
            ItemKind::Pragma(item) => self.visit_pragma_directive(item)?,
            ItemKind::Import(item) => self.visit_import_directive(item)?,
            ItemKind::Using(item) => self.visit_using_directive(item)?,
            ItemKind::Contract(item) => self.visit_item_contract(item)?,
            ItemKind::Function(item_function) => {
                if item_function.kind.is_constructor() {
                    let parent = if self.current_parent.is_empty() {
                        None
                    } else {
                        self.current_parent.last().cloned()
                    };

                    let params = parameters_list_to_identifiers(
                        item_function.header.parameters,
                        self.source_map,
                    );

                    let natspec =
                        extract_natspec(docs, self.source_map, parent.clone(), span).unwrap();

                    self.definitions
                        .push(Definition::Constructor(ConstructorDefinition {
                            parent: parent.clone(),
                            span: span_to_text_range(span, self.source_map),
                            params,
                            natspec,
                        }))
                }

                self.visit_item_function(item_function)?
            }
            ItemKind::Variable(item) => self.visit_variable_definition(item)?,
            ItemKind::Struct(item) => self.visit_item_struct(item)?,
            ItemKind::Enum(item) => self.visit_item_enum(item)?,
            ItemKind::Udvt(item) => self.visit_item_udvt(item)?,
            ItemKind::Error(item) => self.visit_item_error(item)?,
            ItemKind::Event(item) => self.visit_item_event(item)?,
        }

        ControlFlow::Continue(())
    }

    fn visit_item_contract(
        &mut self,
        contract: &'ast ItemContract<'ast>,
    ) -> ControlFlow<Self::BreakValue> {
        let ItemContract {
            kind: _,
            name,
            bases,
            body,
        } = contract;

        self.current_parent
            .push(item_contract_to_parent(contract, self.source_map));

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

// @todo build the utf16 representation too?
fn span_to_text_range(span: &Span, source_map: &SourceMap) -> TextRange {
    let (_, lo_line, lo_col, hi_line, hi_col) = source_map.span_to_location_info(*span);

    let mut start = TextIndex::ZERO;
    start.utf8 = span.lo().to_usize();
    start.line = lo_line - 1; // Convert from 1-based to 0-based
    start.column = lo_col - 1;

    let mut end = TextIndex::ZERO;
    end.utf8 = span.hi().to_usize();
    end.line = hi_line - 1;
    end.column = hi_col - 1;

    start..end
}

fn parameters_list_to_identifiers<'ast>(
    variable_definitions: &[VariableDefinition<'ast>],
    source_map: &SourceMap,
) -> Vec<Identifier> {
    variable_definitions
        .into_iter()
        .map(|var| Identifier {
            name: var.name.map(|name| name.to_string()),
            span: span_to_text_range(&var.span, source_map),
        })
        .collect()
}

fn item_contract_to_parent(contract: &ItemContract, source_map: &SourceMap) -> Parent {
    match contract.kind {
        ContractKind::Contract | ContractKind::AbstractContract => {
            Parent::Contract(contract.name.to_string())
        }
        ContractKind::Library => Parent::Library(contract.name.to_string()),
        ContractKind::Interface => Parent::Interface(contract.name.to_string()),
    }
}

fn extract_natspec(
    docs: &DocComments,
    source_map: &SourceMap,
    parent: Option<Parent>,
    span: &Span,
) -> Result<Option<NatSpec>> {
    let mut combined = NatSpec::default();

    for doc in docs.iter() {
        let snippet =
            source_map
                .span_to_snippet(doc.span)
                .map_err(|e| Error::NatspecParsingError {
                    parent: parent.clone(),
                    span: span_to_text_range(&doc.span, source_map),
                    message: format!("{e:?}"), //.to_string(),
                })?;

        combined.append(&mut parse_comment(&mut snippet.as_str()).map_err(|e| {
            Error::NatspecParsingError {
                parent: parent.clone(),
                span: span_to_text_range(&doc.span, source_map),
                message: format!("{e:?}"), //.to_string(),
            }
        })?);
    }

    if combined == NatSpec::default() {
        Ok(None)
    } else {
        Ok(Some(combined))
    }
}
