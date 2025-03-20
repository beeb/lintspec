//! Solidity parser interface
use std::path::Path;

use crate::{definitions::Definition, error::Result};

pub mod slang;
pub mod solar;

/// The result of parsing and identifying source items in a document
pub struct ParsedDocument {
    /// The list of definitions found in the document
    pub definitions: Vec<Definition>,

    /// The contents of the file, if requested
    pub contents: Option<String>,
}

/// The trait implemented by all parsers
pub trait Parse {
    /// Parse a document at `path` and identify the relevant source items
    fn parse_document(path: impl AsRef<Path>, keep_contents: bool) -> Result<ParsedDocument>;
}
