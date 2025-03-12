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
    config::{Config, FunctionConfig, Req, VariableConfig, WithParamsRules},
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
pub struct ValidationOptions {
    /// Whether overridden, public and external functions should have an `@inheritdoc`
    #[builder(default = true)]
    pub inheritdoc: bool,
    /// Whether to enforce either `@notice` or `@dev` if either or both are required
    #[builder(default)]
    pub notice_or_dev: bool,
    #[builder(default)]
    pub constructors: WithParamsRules,
    #[builder(default)]
    pub enums: WithParamsRules,
    #[builder(default = WithParamsRules::required())]
    pub errors: WithParamsRules,
    #[builder(default = WithParamsRules::required())]
    pub events: WithParamsRules,
    #[builder(default)]
    pub functions: FunctionConfig,
    #[builder(default = WithParamsRules::required())]
    pub modifiers: WithParamsRules,
    #[builder(default)]
    pub structs: WithParamsRules,
    #[builder(default)]
    pub variables: VariableConfig,
}

impl Default for ValidationOptions {
    /// Get default validation options
    fn default() -> Self {
        Self {
            inheritdoc: true,
            notice_or_dev: false,
            constructors: WithParamsRules::default(),
            enums: WithParamsRules::default(),
            errors: WithParamsRules::required(),
            events: WithParamsRules::required(),
            functions: FunctionConfig::default(),
            modifiers: WithParamsRules::required(),
            structs: WithParamsRules::default(),
            variables: VariableConfig::default(),
        }
    }
}

impl From<&Config> for ValidationOptions {
    fn from(value: &Config) -> Self {
        Self {
            inheritdoc: value.lintspec.inheritdoc,
            notice_or_dev: value.lintspec.notice_or_dev,
            constructors: value.constructors.clone(),
            enums: value.enums.clone(),
            errors: value.errors.clone(),
            events: value.events.clone(),
            functions: value.functions.clone(),
            modifiers: value.modifiers.clone(),
            structs: value.structs.clone(),
            variables: value.variables.clone(),
        }
    }
}

/// A trait implemented by [`Definition`][crate::definitions::Definition] to validate the related `NatSpec`
pub trait Validate {
    /// Validate the definition and extract the relevant diagnostics
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics;
}

/// Check a list of params to see if they are documented with a corresponding item in the [`NatSpec`], and generate a
/// diagnostic for each missing one or if there are more than 1 entry per param.
#[must_use]
pub fn check_params(
    natspec: &Option<NatSpec>,
    rule: Req,
    params: &[Identifier],
    default_span: TextRange,
) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    if rule.is_ignored() || (rule.is_required() && params.is_empty()) {
        return res;
    }
    if rule.is_required() {
        let Some(natspec) = natspec else {
            res.extend(params.iter().filter_map(|p| {
                p.name.as_ref().map(|name| Diagnostic {
                    span: p.span.clone(),
                    message: format!("@param {name} is missing"),
                })
            }));
            return res;
        };
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
    } else if let Some(natspec) = natspec {
        // the rule is to forbid `@param`
        if natspec.has_param() {
            res.push(Diagnostic {
                span: default_span,
                message: "@param is forbidden".to_string(),
            });
        }
    }
    res
}

/// Check a list of returns to see if they are documented with a corresponding item in the [`NatSpec`], and generate a
/// diagnostic for each missing one or if there are more than 1 entry per param.
#[allow(clippy::cast_possible_wrap)]
#[must_use]
pub fn check_returns(
    natspec: &Option<NatSpec>,
    rule: Req,
    returns: &[Identifier],
    default_span: TextRange,
    is_var: bool,
) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    if rule.is_ignored() || (rule.is_required() && returns.is_empty()) {
        return res;
    }
    if rule.is_required() {
        let Some(natspec) = natspec else {
            res.extend(returns.iter().enumerate().map(|(idx, r)| {
                let message = if let Some(name) = &r.name {
                    format!("@return {name} is missing")
                } else if is_var {
                    "@return missing".to_string()
                } else {
                    format!("@return missing for unnamed return #{}", idx + 1)
                };
                Diagnostic {
                    span: r.span.clone(),
                    message,
                }
            }));
            return res;
        };
        let mut unnamed_returns = 0;
        let returns_count = natspec.count_all_returns() as isize;
        for (idx, ret) in returns.iter().enumerate() {
            let message = if let Some(name) = &ret.name {
                match natspec.count_return(ret) {
                    0 => format!("@return {name} is missing"),
                    2.. => {
                        format!("@return {name} is present more than once")
                    }
                    _ => {
                        continue;
                    }
                }
            } else {
                unnamed_returns += 1;
                if idx as isize > returns_count - 1 {
                    if is_var {
                        "@return missing".to_string()
                    } else {
                        format!("@return missing for unnamed return #{}", idx + 1)
                    }
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
    } else if let Some(natspec) = natspec {
        // the rule is to forbid `@return`
        if natspec.has_return() {
            res.push(Diagnostic {
                span: default_span,
                message: "@return is forbidden".to_string(),
            });
        }
    }
    res
}

#[must_use]
pub fn check_notice(natspec: &Option<NatSpec>, rule: Req, span: TextRange) -> Option<Diagnostic> {
    // add default `NatSpec` to avoid duplicate match arms for None vs Some with no notice
    match (rule, natspec, &NatSpec::default()) {
        (Req::Required, Some(natspec), _) | (Req::Required, None, natspec)
            if !natspec.has_notice() =>
        {
            Some(Diagnostic {
                span,
                message: "@notice is missing".to_string(),
            })
        }
        (Req::Forbidden, Some(natspec), _) if natspec.has_notice() => Some(Diagnostic {
            span,
            message: "@notice is forbidden".to_string(),
        }),
        _ => None,
    }
}

#[must_use]
pub fn check_dev(natspec: &Option<NatSpec>, rule: Req, span: TextRange) -> Option<Diagnostic> {
    // add default `NatSpec` to avoid duplicate match arms for None vs Some with no dev
    match (rule, natspec, &NatSpec::default()) {
        (Req::Required, Some(natspec), _) | (Req::Required, None, natspec)
            if !natspec.has_dev() =>
        {
            Some(Diagnostic {
                span,
                message: "@dev is missing".to_string(),
            })
        }
        (Req::Forbidden, Some(natspec), _) if natspec.has_dev() => Some(Diagnostic {
            span,
            message: "@dev is forbidden".to_string(),
        }),
        _ => None,
    }
}

#[must_use]
pub fn check_notice_and_dev(
    natspec: &Option<NatSpec>,
    notice_rule: Req,
    dev_rule: Req,
    notice_or_dev: bool,
    span: TextRange,
) -> Vec<Diagnostic> {
    let mut res = Vec::new();
    match (notice_or_dev, notice_rule, dev_rule) {
        (true, Req::Required, Req::Ignored | Req::Required)
        | (true, Req::Ignored, Req::Required) => {
            if natspec.is_none()
                || (!natspec.as_ref().unwrap().has_notice() && !natspec.as_ref().unwrap().has_dev())
            {
                res.push(Diagnostic {
                    span,
                    message: "@notice or @dev is missing".to_string(),
                });
            }
        }
        (true, Req::Forbidden, _) | (true, _, Req::Forbidden) | (false, _, _) => {
            res.extend(check_notice(natspec, notice_rule, span.clone()));
            res.extend(check_dev(natspec, dev_rule, span));
        }
        (true, Req::Ignored, Req::Ignored) => {}
    }
    res
}
