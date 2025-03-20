//! Solidity parser interface
use std::path::Path;

use crate::{
    definitions::Definition,
    error::Result,
    parser::{Parse, ParsedDocument},
};
use solar_parse::ast::{visit::Visit, PragmaDirective};
use std::ops::ControlFlow;
use winnow::combinator::todo;

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

impl Parse for SolarParser {
    fn parse_document(path: impl AsRef<Path>, keep_contents: bool) -> Result<ParsedDocument> {
        todo!()
    }
}

impl<'ast> Visit<'ast> for SolarParser {
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
