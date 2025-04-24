//! Solidity parser interface
use std::io;

use crate::{definitions::Definition, error::Result};

pub mod slang;

/// The result of parsing and identifying source items in a document
#[derive(Debug)]
pub struct ParsedDocument {
    /// The list of definitions found in the document
    pub definitions: Vec<Definition>,

    /// The contents of the file, if requested
    pub contents: Option<String>,
}

/// The trait implemented by all parsers
pub trait Parse {
    /// Parse a document from a reader and identify the relevant source items
    ///
    /// The fact that this takes in a mutable reference to the parser allows for stateful parsers.
    fn parse_document(
        &mut self,
        input: impl io::Read,
        keep_contents: bool,
    ) -> Result<ParsedDocument>;
}
