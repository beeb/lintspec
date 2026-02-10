//! Solidity parser interface
use std::{collections::HashMap, io, path::Path};

use crate::{definitions::Definition, error::Result};

#[cfg_attr(docsrs, doc(cfg(feature = "slang")))]
#[cfg(feature = "slang")]
pub mod slang;

#[cfg_attr(docsrs, doc(cfg(feature = "solar")))]
#[cfg(feature = "solar")]
pub mod solar;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DocumentId(u64);

impl DocumentId {
    /// Generate a new random and unique document ID
    #[must_use]
    pub fn new() -> Self {
        DocumentId(fastrand::u64(..))
    }
}

/// The result of parsing and identifying source items in a document
#[derive(Debug)]
pub struct ParsedDocument {
    /// A unique ID for the document given by the parser
    ///
    /// Can be used to retrieve the document contents after parsing (via [`Parse::get_sources`]).
    pub id: DocumentId,

    /// The list of definitions found in the document
    pub definitions: Vec<Definition>,
}

/// The trait implemented by all parsers
///
/// Ideally, cloning a parser should not duplicate the contents of the sources. The underlying data should be wrapped
/// in an [`Arc`][std::sync::Arc], so that the last clone of a parser is able to retrieve the sources' contents for
/// all files.
pub trait Parse: Clone {
    /// Parse a document from a reader and identify the relevant source items
    ///
    /// If a path is provided, then this can be used to enrich diagnostics.
    /// The fact that this takes in a mutable reference to the parser allows for stateful parsers.
    fn parse_document(
        &mut self,
        input: impl io::Read,
        path: Option<impl AsRef<Path>>,
        keep_contents: bool,
    ) -> Result<ParsedDocument>;

    /// Retrieve the contents of the source files after parsing is done
    ///
    /// This consumes the parser, so that ownership of the contents can be retrieved safely.
    /// Note that documents which were parsed with `keep_contents` to `false` will no be present in the map.
    ///
    /// This can return an error if there are more than one clone of the parser.
    fn get_sources(self) -> Result<HashMap<DocumentId, String>>;
}
