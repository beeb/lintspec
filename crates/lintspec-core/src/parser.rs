//! Solidity parser interface
use std::{collections::HashMap, io, path::Path};

use crate::{
    definitions::{
        Definition, constructor::ConstructorDefinition, enumeration::EnumDefinition,
        error::ErrorDefinition, event::EventDefinition, modifier::ModifierDefinition,
        structure::StructDefinition,
    },
    error::{ErrorKind, Result},
    prelude::OrPanic as _,
    textindex::{TextIndex, TextRange, compute_indices},
};

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

/// Gather all the start and end byte offsets for the definitions, including their members/params/returns.
///
/// This requires that the definitions are sorted by start offset. The result is also sorted.
fn gather_offsets(definitions: &[Definition]) -> Vec<usize> {
    fn register_span(offsets: &mut Vec<usize>, span: &TextRange) {
        offsets.push(span.start.utf8);
        offsets.push(span.end.utf8);
    }
    let mut offsets = Vec::with_capacity(definitions.len() * 16); // seems about right from code in the wild
    // register all start and end utf-8 offsets for the definitions and their relevant properties
    // definitions are sorted by start offset due to how the AST is traversed
    for def in definitions {
        def.span().inspect(|s| register_span(&mut offsets, s));
        match def {
            Definition::Constructor(ConstructorDefinition { params, .. })
            | Definition::Error(ErrorDefinition { params, .. })
            | Definition::Event(EventDefinition { params, .. })
            | Definition::Modifier(ModifierDefinition { params, .. })
            | Definition::Enumeration(EnumDefinition {
                members: params, ..
            })
            | Definition::Struct(StructDefinition {
                members: params, ..
            }) => {
                for p in params {
                    register_span(&mut offsets, &p.span);
                }
            }
            Definition::Function(d) => {
                d.params
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
                d.returns
                    .iter()
                    .for_each(|i| register_span(&mut offsets, &i.span));
            }
            Definition::NatspecParsingError(ErrorKind::NatspecParsingError { span, .. }) => {
                register_span(&mut offsets, span);
            }
            Definition::Contract(_)
            | Definition::Interface(_)
            | Definition::Library(_)
            | Definition::Variable(_)
            | Definition::NatspecParsingError(_) => {}
        }
    }
    // we might have duplicate offsets and they are out of order (because a struct definition's span end is greater than
    // the span start of its first member for example)
    // we will deduplicate on the fly as we iterate to avoid re-allocating
    offsets.sort_unstable();
    offsets
}

/// Fill in the missing values in the spans of definitions.
fn populate(text_indices: &[TextIndex], definitions: &mut Vec<Definition>) {
    fn populate_span(indices: &[TextIndex], start_idx: usize, span: &mut TextRange) -> usize {
        let idx;
        (idx, span.start) = indices
            .iter()
            .enumerate()
            .skip(start_idx)
            .find_map(|(i, ti)| (ti.utf8 >= span.start.utf8).then_some((i, *ti)))
            .or_panic("utf8 start offset should be present in cache");
        span.end = *indices
            .iter()
            .skip(idx + 1)
            .find(|ti| ti.utf8 >= span.end.utf8)
            .or_panic("utf8 end offset should be present in cache");
        // for the next definition or item inside of a definition, we can start after the start of this item
        // because start indices increase monotonically
        idx + 1
    }
    // definitions are sorted by start offset due to how the AST is traversed
    // likewise, params, members, etc., are also sorted by start offset
    // this means that we can populate spans while ignoring all items in `text_indices` prior to the index corresponding
    // to the start offset of the previous definition
    let mut idx = 0;
    for def in definitions {
        if let Some(span) = def.span_mut() {
            idx = populate_span(text_indices, idx, span);
        }
        match def {
            Definition::Constructor(ConstructorDefinition { params, .. })
            | Definition::Error(ErrorDefinition { params, .. })
            | Definition::Event(EventDefinition { params, .. })
            | Definition::Modifier(ModifierDefinition { params, .. })
            | Definition::Enumeration(EnumDefinition {
                members: params, ..
            })
            | Definition::Struct(StructDefinition {
                members: params, ..
            }) => {
                for p in params {
                    idx = populate_span(text_indices, idx, &mut p.span);
                }
            }
            Definition::Function(d) => {
                for p in &mut d.params {
                    idx = populate_span(text_indices, idx, &mut p.span);
                }
                for p in &mut d.returns {
                    idx = populate_span(text_indices, idx, &mut p.span);
                }
            }
            Definition::NatspecParsingError(ErrorKind::NatspecParsingError { span, .. }) => {
                idx = populate_span(text_indices, idx, span);
            }
            Definition::Contract(_)
            | Definition::Interface(_)
            | Definition::Library(_)
            | Definition::Variable(_)
            | Definition::NatspecParsingError(_) => {}
        }
    }
}

/// Complete the [`TextRange`] of a list of [`Definition`].
///
/// Parsers might only give us partial information (e.g. utf-8 byte offsets), but we need the full
/// line/column information in all encodings. This function computes the missing fields.
pub fn complete_text_ranges(source: &str, definitions: &mut Vec<Definition>) {
    let offsets = gather_offsets(definitions);
    if offsets.is_empty() {
        return;
    }

    let text_indices = compute_indices(source, &offsets);

    populate(&text_indices, definitions);
}
