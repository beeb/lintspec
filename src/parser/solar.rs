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

trait Extract<'ast> {
    fn extract_definition(
        visitor: &mut LintspecVisitor<'ast>,
        item: &'ast Item<'ast>,
    ) -> Option<Definition>;
}

impl<'ast> Extract<'ast> for solar_parse::ast::ItemFunction<'ast> {
    fn extract_definition(
        visitor: &mut LintspecVisitor<'ast>,
        item: &'ast Item<'ast>,
    ) -> Option<Definition> {
        if let ItemKind::Function(item_function) = &item.kind {
            let parent = visitor.current_parent.last().cloned();
            let params =
                parameters_list_to_identifiers(item_function.header.parameters, visitor.source_map);
            let natspec =
                extract_natspec(&item.docs, visitor.source_map, parent.clone(), &item.span).ok()?;
            let span = span_to_text_range(&item.span, visitor.source_map);

            let def = if item_function.kind.is_constructor() {
                Definition::Constructor(ConstructorDefinition {
                    parent,
                    span,
                    params,
                    natspec,
                })
            } else if item_function.kind.is_function() {
                Definition::Function(FunctionDefinition {
                    parent,
                    name: item_function.header.name.unwrap().to_string(),
                    returns: returns_to_identifiers(
                        item_function.header.returns,
                        visitor.source_map,
                    ),
                    attributes: Attributes {
                        visibility: item_function.header.visibility.into(),
                        r#override: item_function.header.override_.is_some(),
                    },
                    span,
                    params,
                    natspec,
                })
            } else if item_function.kind.is_modifier() {
                Definition::Modifier(ModifierDefinition {
                    parent,
                    span,
                    params,
                    natspec,
                    name: item_function.header.name.unwrap().to_string(),
                    attributes: Attributes {
                        visibility: item_function.header.visibility.into(),
                        r#override: item_function.header.override_.is_some(),
                    },
                })
            } else {
                return None;
            };

            Some(def)
        } else {
            None
        }
    }
}

impl<'ast> Extract<'ast> for solar_parse::ast::VariableDefinition<'ast> {
    fn extract_definition(
        visitor: &mut LintspecVisitor<'ast>,
        item: &'ast Item<'ast>,
    ) -> Option<Definition> {
        let Item { docs, span, kind } = item;
        let parent = visitor.current_parent.last().cloned();

        if let ItemKind::Variable(item_variable) = &item.kind {
            let text_range = span_to_text_range(span, visitor.source_map);

            let natspec = extract_natspec(docs, visitor.source_map, parent.clone(), span).unwrap();

            let attributes = Attributes {
                visibility: item_variable.visibility.into(),
                r#override: item_variable.override_.is_some(),
            };

            Some(Definition::Variable(VariableDeclaration {
                parent,
                name: item_variable.name.unwrap().to_string(),
                span: text_range,
                natspec,
                attributes,
            }))
        } else {
            None
        }
    }
}

impl<'ast> Extract<'ast> for solar_parse::ast::ItemStruct<'ast> {
    fn extract_definition(
        visitor: &mut LintspecVisitor<'ast>,
        item: &'ast Item<'ast>,
    ) -> Option<Definition> {
        let Item { docs, span, kind } = item;
        let parent = visitor.current_parent.last().cloned();

        if let ItemKind::Struct(strukt) = &item.kind {
            let name = strukt.name.to_string();

            let members = strukt
                .fields
                .iter()
                .map(|m| Identifier {
                    name: Some(m.name.expect("name").to_string()),
                    span: span_to_text_range(&m.span, visitor.source_map),
                })
                .collect();

            let natspec = extract_natspec(docs, visitor.source_map, parent.clone(), span).unwrap();

            Some(Definition::Struct(StructDefinition {
                parent,
                name,
                span: span_to_text_range(&docs.span(), visitor.source_map),
                members,
                natspec,
            }))
        } else {
            None
        }
    }
}

impl<'ast> Extract<'ast> for solar_parse::ast::ItemEnum<'ast> {
    fn extract_definition(
        visitor: &mut LintspecVisitor<'ast>,
        item: &'ast Item<'ast>,
    ) -> Option<Definition> {
        let Item { docs, span, kind } = item;
        let parent = visitor.current_parent.last().cloned();

        if let ItemKind::Enum(item_enum) = &item.kind {
            let members = item_enum
                .variants
                .iter()
                .map(|v| Identifier {
                    name: Some(v.name.to_string()),
                    span: span_to_text_range(&v.span, visitor.source_map),
                })
                .collect();

            Some(Definition::Enumeration(EnumDefinition {
                parent: parent.clone(),
                name: item_enum.name.to_string(),
                span: span_to_text_range(span, visitor.source_map),
                members,
                natspec: extract_natspec(docs, visitor.source_map, parent.clone(), span).unwrap(),
            }))
        } else {
            None
        }
    }
}

impl<'ast> Extract<'ast> for solar_parse::ast::ItemError<'ast> {
    fn extract_definition(
        visitor: &mut LintspecVisitor<'ast>,
        item: &'ast Item<'ast>,
    ) -> Option<Definition> {
        let Item { docs, span, kind } = item;
        let parent = visitor.current_parent.last().cloned();

        if let ItemKind::Error(item_error) = &item.kind {
            let params = parameters_list_to_identifiers(item_error.parameters, visitor.source_map);

            let natspec = extract_natspec(docs, visitor.source_map, parent.clone(), span).unwrap();

            Some(Definition::Error(ErrorDefinition {
                parent: parent.clone(),
                span: span_to_text_range(span, visitor.source_map),
                name: item_error.name.to_string(),
                params,
                natspec,
            }))
        } else {
            None
        }
    }
}

impl<'ast> Extract<'ast> for solar_parse::ast::ItemEvent<'ast> {
    fn extract_definition(
        visitor: &mut LintspecVisitor<'ast>,
        item: &'ast Item<'ast>,
    ) -> Option<Definition> {
        let Item { docs, span, kind } = item;
        let parent = visitor.current_parent.last().cloned();

        if let ItemKind::Event(item_event) = &item.kind {
            let params =
                parameters_list_to_identifiers(item_eventitem.parameters, visitor.source_map);

            let natspec = extract_natspec(docs, visitor.source_map, parent.clone(), span).unwrap();

            Some(Definition::Event(EventDefinition {
                parent: parent.clone(),
                name: item_event.name.to_string(),
                span: span_to_text_range(span, visitor.source_map),
                params,
                natspec,
            }))
        } else {
            None
        }
    }
}

impl Parse for SolarParser {
    fn parse_document(path: impl AsRef<Path>, keep_contents: bool) -> Result<ParsedDocument> {
        let path = path.as_ref().to_path_buf();

        let sess = Session::builder().with_silent_emitter(None).build();
        let source_map = sess.source_map();

        let definitions = sess.enter(|| -> solar_parse::interface::Result<Vec<Definition>> {
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

            Ok(visitor.definitions)
        });

        Ok(ParsedDocument {
            definitions: definitions.unwrap(), // unwrap error guaranteed from the session
            contents: None,
        })
    }
}

impl<'ast> Visit<'ast> for LintspecVisitor<'ast> {
    type BreakValue = ();

    fn visit_item(&mut self, item: &'ast Item<'ast>) -> ControlFlow<Self::BreakValue> {
        let Item { docs, span, kind } = item;
        self.visit_span(span)?;
        self.visit_doc_comments(docs)?;

        let parent = self.current_parent.last().cloned();

        match kind {
            ItemKind::Pragma(item) => self.visit_pragma_directive(item)?,
            ItemKind::Import(item) => self.visit_import_directive(item)?,
            ItemKind::Using(item) => self.visit_using_directive(item)?,
            ItemKind::Contract(item) => self.visit_item_contract(item)?,
            ItemKind::Function(item_function) => {
                if let Some(def) =
                    <solar_parse::ast::ItemFunction as Extract>::extract_definition(self, item)
                {
                    self.definitions.push(def);
                }

                self.visit_item_function(item_function)?
            }
            ItemKind::Variable(var_def) => {
                if let Some(def) =
                    <solar_parse::ast::VariableDefinition as Extract>::extract_definition(
                        self, item,
                    )
                {
                    self.definitions.push(def);
                }

                self.visit_variable_definition(var_def)?
            }
            ItemKind::Struct(strukt) => {
                if let Some(struct_def) =
                    <solar_parse::ast::ItemStruct as Extract>::extract_definition(self, item)
                {
                    self.definitions.push(struct_def);
                }

                self.visit_item_struct(item)?
            }
            ItemKind::Enum(item_enum) => {
                if let Some(enum_def) =
                    <solar_parse::ast::ItemEnum as Extract>::extract_definition(self, item)
                {
                    self.definitions.push(enum_def);
                }

                self.visit_item_enum(item_enum)?
            }
            ItemKind::Udvt(item) => self.visit_item_udvt(item)?,
            ItemKind::Error(item_error) => {
                if let Some(def) =
                    <solar_parse::ast::ItemError as Extract>::extract_definition(self, item)
                {
                    self.definitions.push(def);
                }

                self.visit_item_error(item_error)?
            }
            ItemKind::Event(item_event) => {
                if let Some(def) =
                    <solar_parse::ast::ItemEvent as Extract>::extract_definition(self, item)
                {
                    self.definitions.push(def);
                }

                self.visit_item_event(item_event)?
            }
        }

        ControlFlow::Continue(())
    }

    // Needed to track parent:
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

fn returns_to_identifiers(
    returns: &[VariableDefinition],
    source_map: &SourceMap,
) -> Vec<Identifier> {
    returns
        .iter()
        .map(|r| Identifier {
            name: Some(r.name.unwrap_or_default().to_string()), // @todo anonymous return are empty?
            span: span_to_text_range(&r.span, source_map),
        })
        .collect()
}

impl From<Option<solar_parse::ast::Visibility>> for Visibility {
    fn from(visibility: Option<solar_parse::ast::Visibility>) -> Self {
        match visibility {
            Some(solar_parse::ast::Visibility::Public) => Visibility::Public,
            Some(solar_parse::ast::Visibility::Internal) => Visibility::Internal,
            Some(solar_parse::ast::Visibility::Private) => Visibility::Private,
            Some(solar_parse::ast::Visibility::External) => Visibility::External,
            None => Visibility::Internal,
        }
    }
}
