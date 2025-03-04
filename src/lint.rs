//! Check the `NatSpec` documentation of a source file
//!
//! The [`lint`] function parsers the source file and contained items, validates them according to the configured
//! rules and emits a list of diagnostics, grouped by source item.
use std::{
    io,
    path::{Path, PathBuf},
};

use serde::Serialize;
use slang_solidity::cst::TextRange;

use crate::{
    config::Config,
    definitions::{Identifier, ItemType, Parent},
    error::Result,
    natspec::NatSpec,
    parser::Parse,
};

/// Diagnostics for a single Solidity file
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("Error")]
pub struct FileDiagnostics {
    /// Path to the file
    pub path: PathBuf,

    /// Contents of the file (can be an empty string if pretty output is not activated)
    #[serde(skip_serializing)]
    pub contents: Option<String>,

    /// Diagnostics, grouped by source item (function, struct, etc.)
    pub items: Vec<ItemDiagnostics>,
}

/// Diagnostics for a single source item (function, struct, etc.)
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct ItemDiagnostics {
    /// The parent contract, interface or library's name, if any
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

/// A single diagnostic related to `NatSpec`.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// The span (text range) related to the diagnostic
    ///
    /// If related to the item's `NatSpec` as a whole, this is the same as the item's span.
    /// For a missing param or return `NatSpec`, this is the span of the param or return item.
    pub span: TextRange,
    pub message: String,
}

/// Lint a file by identifying `NatSpec` problems.
///
/// This is the main business logic entrypoint related to using this library. The path to the Solidity file should be
/// provided, and a compatible Solidity version will be inferred from the first version pragma statement (if any) to
/// inform the parsing. [`ValidationOptions`] can be provided to control whether some of the lints get reported.
/// The `keep_contents` parameter controls if the returned [`FileDiagnostics`] contains the original source code.
pub fn lint<T>(
    path: impl AsRef<Path>,
    options: &ValidationOptions,
    keep_contents: bool,
) -> Result<Option<FileDiagnostics>>
where
    T: Parse,
{
    let document = T::parse_document(&path, keep_contents)?;
    let mut items: Vec<_> = document
        .definitions
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
        contents: document.contents,
        items,
    }))
}

/// Validation options to control which lints generate a diagnostic
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)]
pub struct ValidationOptions {
    /// Whether overridden, public and external functions should have an `@inheritdoc`
    #[builder(default = true)]
    pub inheritdoc: bool,

    /// Whether to check that constructors have documented params
    #[builder(default = false)]
    pub constructor: bool,

    /// Whether to check that each member of structs is documented with `@param`
    ///
    /// Not standard practice, the Solidity spec does not consider `@param` for structs or provide any other way to
    /// document each member.
    #[builder(default = false)]
    pub struct_params: bool,

    /// Whether to check that each variant of enums is documented with `@param`
    ///
    /// Not standard practice, the Solidity spec does not consider `@param` for enums or provide any other way to
    /// document each variant.
    #[builder(default = false)]
    pub enum_params: bool,

    /// Which item types should have at least some [`NatSpec`] even if they have no param/return/member.
    #[builder(default = vec![])]
    pub enforce: Vec<ItemType>,
}

impl Default for ValidationOptions {
    /// Get default validation options (`inheritdoc` is true, the others are false)
    fn default() -> Self {
        Self {
            inheritdoc: true,
            constructor: false,
            struct_params: false,
            enum_params: false,
            enforce: vec![],
        }
    }
}

impl From<&Config> for ValidationOptions {
    fn from(value: &Config) -> Self {
        Self {
            inheritdoc: value.inheritdoc,
            constructor: value.constructor,
            struct_params: value.struct_params,
            enum_params: value.enum_params,
            enforce: value.enforce.clone(),
        }
    }
}

/// A trait implemented by [`Definition`] to validate the related `NatSpec`
pub trait Validate {
    /// Validate the definition and extract the relevant diagnostics
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics;
}

/// Check a list of params to see if they are documented with a corresponding item in the [`NatSpec`], and generate a
/// diagnostic for each missing one or if there are more than 1 entry per param.
#[must_use]
pub fn check_params(natspec: &NatSpec, params: &[Identifier]) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    for param in params {
        let Some(name) = &param.name else {
            // no need to document unused params
            continue;
        };
        let message = match natspec.count_param(param) {
            0 => format!("@param {name} is missing"),
            2.. => format!("@param {name} is present more than once"),
            _ => {
                continue;
            }
        };
        res.push(Diagnostic {
            span: param.span.clone(),
            message,
        });
    }
    res
}

/// Check a list of returns to see if they are documented with a corresponding item in the [`NatSpec`], and generate a
/// diagnostic for each missing one or if there are more than 1 entry per param.
#[allow(clippy::cast_possible_wrap)]
#[must_use]
pub fn check_returns(
    natspec: &NatSpec,
    returns: &[Identifier],
    default_span: TextRange,
) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    let mut unnamed_returns = 0;
    let returns_count = natspec.count_all_returns() as isize;
    for (idx, ret) in returns.iter().enumerate() {
        let message = if let Some(name) = &ret.name {
            match natspec.count_return(ret) {
                0 => format!("@return {name} is missing"),
                2.. => format!("@return {name} is present more than once"),
                _ => {
                    continue;
                }
            }
        } else {
            unnamed_returns += 1;
            if idx as isize > returns_count - 1 {
                format!("@return missing for unnamed return #{}", idx + 1)
            } else {
                continue;
            }
        };
        res.push(Diagnostic {
            span: ret.span.clone(),
            message,
        });
    }
    if natspec.count_unnamed_returns() > unnamed_returns {
        res.push(Diagnostic {
            span: returns.last().cloned().map_or(default_span, |r| r.span),
            message: "too many unnamed returns".to_string(),
        });
    }
    res
}
