use std::{
    fs, io,
    path::{Path, PathBuf},
};

use derive_more::Display;
use serde::Serialize;
use slang_solidity::{
    cst::{NonterminalKind, TextRange},
    parser::Parser,
};

use crate::{
    definitions::{find_items, Parent, ValidationOptions},
    error::{Error, Result},
    utils::detect_solidity_version,
};

/// Diagnostics for a single Solidity file
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("Error")]
pub struct FileDiagnostics {
    /// Path to the file
    pub path: PathBuf,

    /// Contents of the file (can be an empty string if pretty output is not activated)
    #[serde(skip_serializing)]
    pub contents: String,

    /// Diagnostics, grouped by source item (function, struct, etc.)
    pub items: Vec<ItemDiagnostics>,
}

/// Diagnostics for a single source item (function, struct, etc.)
#[derive(Debug, Clone, Serialize)]
pub struct ItemDiagnostics {
    /// The parent contract, interface or library's name
    pub parent: Option<Parent>,

    /// The type of this source item (function, struct, etc.)
    pub item_type: ItemType,

    /// The name of the item
    pub name: String,

    /// The span of the item (for function-like items, only the declaration without the body)
    pub span: TextRange,

    /// The diagnostics related to this item
    pub diags: Vec<Diagnostic>,
}

impl ItemDiagnostics {
    /// Print the diagnostics for a single source item in a compact format
    ///
    /// The writer `f` can be stderr or a file handle, for example. The path to the file containing this source item
    /// should be provided, as well as the current working directory where the command was launched from. This last
    /// information is used to compute the relative path when possible.
    pub fn print_compact(
        &self,
        f: &mut impl io::Write,
        path: impl AsRef<Path>,
        root_dir: impl AsRef<Path>,
    ) -> std::result::Result<(), io::Error> {
        let source_name = match path.as_ref().strip_prefix(root_dir.as_ref()) {
            Ok(relative_path) => relative_path.to_string_lossy(),
            Err(_) => path.as_ref().to_string_lossy(),
        };
        writeln!(
            f,
            "{source_name}:{}:{}",
            self.span.start.line + 1,   // lines start at zero in the span
            self.span.start.column + 1, // columsn start at zero in the span
        )?;
        if let Some(parent) = &self.parent {
            writeln!(f, "{} {}.{}", self.item_type, parent, self.name)?;
        } else {
            writeln!(f, "{} {}", self.item_type, self.name)?;
        }
        for diag in &self.diags {
            writeln!(f, "  {}", diag.message)?;
        }
        writeln!(f)?;
        Ok(())
    }
}

/// A type of source item (function, struct, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Display)]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
    #[display("constructor")]
    Constructor,
    #[display("enum")]
    Enum,
    #[display("error")]
    Error,
    #[display("event")]
    Event,
    #[display("function")]
    Function,
    #[display("modifier")]
    Modifier,

    ParsingError,
    #[display("struct")]
    Struct,
    #[display("variable")]
    Variable,
}

/// A single diagnostic related to NatSpec.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// The span (text range) related to the diagnostic
    ///
    /// If related to the item's NatSpec as a whole, this is the same as the item's span.
    /// For a missing param or return NatSpec, this is the span of the param or return item.
    pub span: TextRange,
    pub message: String,
}

/// Lint a file by identifying NatSpec problems.
///
/// This is the main business logic entrypoint related to using this library. The path to the Solidity file should be
/// provided, and a compatible Solidity version will be inferred from the first version pragma statement (if any) to
/// inform the parsing. [`ValidationOptions`] can be provided to control whether some of the lints get reported.
/// The `drop_contents` parameter controls if the returned [`FileDiagnostics`] contains the original source code
/// (false) or an empty string (true).
pub fn lint(
    path: impl AsRef<Path>,
    options: &ValidationOptions,
    drop_contents: bool,
) -> Result<Option<FileDiagnostics>> {
    let mut contents = fs::read_to_string(&path).map_err(|err| Error::IOError {
        path: path.as_ref().to_path_buf(),
        err,
    })?;
    let solidity_version = detect_solidity_version(&contents)?;

    let parser = Parser::create(solidity_version).expect("parser should initialize");

    let output = parser.parse(NonterminalKind::SourceUnit, &contents);
    if drop_contents {
        // free memory
        contents = String::new();
    }
    if !output.is_valid() {
        let Some(error) = output.errors().first() else {
            return Err(Error::UnknownError);
        };
        return Err(Error::ParsingError(error.to_string()));
    }

    let cursor = output.create_tree_cursor();
    let mut items: Vec<_> = find_items(cursor)
        .into_iter()
        .filter_map(|item| {
            let mut item_diags = item.validate(options);
            if item_diags.diags.is_empty() {
                None
            } else {
                item_diags.diags.sort_unstable_by_key(|d| d.span.start);
                Some(item_diags)
            }
        })
        .collect();
    if items.is_empty() {
        return Ok(None);
    }
    items.sort_unstable_by_key(|i| i.span.start);
    Ok(Some(FileDiagnostics {
        path: path.as_ref().to_path_buf(),
        contents,
        items,
    }))
}
