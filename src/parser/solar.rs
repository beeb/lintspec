//! Solidity parser interface
use std::path::Path;
use std::fs;

use solar_parse::{
    ast::{interface::{source_map::FileName, Session}, visit::Visit, PragmaDirective},
    Parser
};

use std::ops::ControlFlow;

use crate::{
    definitions::{
        constructor::ConstructorDefinition, enumeration::EnumDefinition, error::ErrorDefinition,
        event::EventDefinition, function::FunctionDefinition, modifier::ModifierDefinition,
        structure::StructDefinition, variable::VariableDeclaration, Attributes, Definition,
        Identifier, Parent, Visibility,
    },
    error::{Error, Result},
    natspec::{parse_comment, NatSpec},
    utils::detect_solidity_version,
    parser::{Parse, ParsedDocument}
};

// /// The result of parsing and identifying source items in a document
// pub struct ParsedDocument {
//     /// The list of definitions found in the document
//     pub definitions: Vec<Definition>,

//     /// The contents of the file, if requested
//     pub contents: Option<String>,
// }

// /// The trait implemented by all parsers
// pub trait Parse {
//     /// Parse a document at `path` and identify the relevant source items
//     fn parse_document(path: impl AsRef<Path>, keep_contents: bool) -> Result<ParsedDocument>;
// }

pub struct SolarParser {}

pub struct LintspecVisitor {}

impl Parse for SolarParser {
    fn parse_document(path: impl AsRef<Path>, keep_contents: bool) -> Result<ParsedDocument> {
        let path = path.as_ref().to_path_buf();

        let sess = Session::builder().with_silent_emitter(None).build();

        let _ = sess.enter(|| -> solar_parse::interface::Result<()> {
            let arena = solar_parse::ast::Arena::new();

            let mut parser =
                Parser::from_lazy_source_code(&sess, &arena, FileName::from(path.clone()), || {
                    fs::read_to_string(&path)
                })?;

            let ast = parser.parse_file().map_err(|e| e.emit())?;

            let mut visitor = LintspecVisitor {};

            let result = visitor.visit_source_unit(&ast);

            Ok(())
        });

        Ok(ParsedDocument { definitions: vec![], contents: None })
    }
}

impl<'ast> Visit<'ast> for LintspecVisitor {
    type BreakValue = ();

    fn visit_pragma_directive(
        &mut self,
        pragma: &'ast PragmaDirective<'ast>,
    ) -> ControlFlow<Self::BreakValue> {
        // noop by default.
        let PragmaDirective { tokens: _ } = pragma;
        ControlFlow::Continue(())
    }
}
