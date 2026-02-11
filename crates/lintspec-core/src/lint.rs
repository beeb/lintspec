//! Check the `NatSpec` documentation of a source file
//!
//! The [`lint`] function parsers the source file and contained items, validates them according to the configured
//! rules and emits a list of diagnostics, grouped by source item.
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::{
    config::{Config, ContractRules, FunctionConfig, Req, VariableConfig, WithParamsRules},
    definitions::{Identifier, ItemType, Parent},
    error::{Error, Result},
    interner::INTERNER,
    natspec::{NatSpec, NatSpecKind},
    parser::{DocumentId, Parse, ParsedDocument},
    textindex::TextRange,
};

/// Diagnostics for a single Solidity file
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("Error")]
pub struct FileDiagnostics {
    /// Path to the file
    pub path: PathBuf,

    /// A unique ID for the document given by the parser
    ///
    /// Can be used to retrieve the document contents after parsing (via [`Parse::get_sources`]).
    #[serde(skip_serializing)]
    pub document_id: DocumentId,

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
    pub name: &'static str,

    /// The span of the item (for function-like items, only the declaration without the body)
    pub span: TextRange,

    /// The diagnostics related to this item
    pub diags: Vec<Diagnostic>,
}

impl ItemDiagnostics {
    /// Print the diagnostics for a single source item in a compact format
    ///
    /// The writer `f` can be stderr or a file handle, for example. The `source_name` should be
    /// the display name for the file (typically a relative path from the working directory).
    pub fn print_compact(
        &self,
        f: &mut impl io::Write,
        source_name: &str,
    ) -> std::result::Result<(), io::Error> {
        writeln!(f, "{source_name}:{}", self.span.start)?;
        if let Some(parent) = &self.parent {
            writeln!(f, "{} {}.{}", self.item_type, parent, self.name)?;
        } else {
            writeln!(f, "{} {}", self.item_type, self.name)?;
        }
        for diag in &self.diags {
            writeln!(f, "  {}", diag.message)?;
        }
        writeln!(f)
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
pub fn lint(
    mut parser: impl Parse,
    path: impl AsRef<Path>,
    options: &ValidationOptions,
    keep_contents: bool,
) -> Result<Option<FileDiagnostics>> {
    fn inner(
        path: &Path,
        document: ParsedDocument,
        options: &ValidationOptions,
    ) -> Option<FileDiagnostics> {
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
            return None;
        }
        items.sort_unstable_by_key(|i| i.span.start);
        Some(FileDiagnostics {
            path: path.to_path_buf(),
            document_id: document.id,
            items,
        })
    }
    let file = File::open(&path).map_err(|err| Error::IOError {
        path: path.as_ref().to_path_buf(),
        err,
    })?;
    let document = parser.parse_document(file, Some(&path), keep_contents)?;
    Ok(inner(path.as_ref(), document, options))
}

/// Validation options to control which lints generate a diagnostic
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct ValidationOptions {
    /// Whether public and external functions should have an `@inheritdoc`
    #[builder(default = true)]
    pub inheritdoc: bool,

    /// Whether `override` internal functions and modifiers should have an `@inheritdoc`
    #[builder(default = false)]
    pub inheritdoc_override: bool,

    /// Whether to enforce either `@notice` or `@dev` if either or both are required
    #[builder(default)]
    pub notice_or_dev: bool,

    /// Validation options for contracts
    #[builder(default)]
    pub contracts: ContractRules,

    /// Validation options for interfaces
    #[builder(default)]
    pub interfaces: ContractRules,

    /// Validation options for libraries
    #[builder(default)]
    pub libraries: ContractRules,

    /// Validation options for constructors
    #[builder(default = WithParamsRules::default_constructor())]
    pub constructors: WithParamsRules,

    /// Validation options for enums
    #[builder(default)]
    pub enums: WithParamsRules,

    /// Validation options for errors
    #[builder(default = WithParamsRules::required())]
    pub errors: WithParamsRules,

    /// Validation options for events
    #[builder(default = WithParamsRules::required())]
    pub events: WithParamsRules,

    /// Validation options for functions
    #[builder(default)]
    pub functions: FunctionConfig,

    /// Validation options for modifiers
    #[builder(default = WithParamsRules::required())]
    pub modifiers: WithParamsRules,

    /// Validation options for structs
    #[builder(default)]
    pub structs: WithParamsRules,

    /// Validation options for state variables
    #[builder(default)]
    pub variables: VariableConfig,
}

impl Default for ValidationOptions {
    /// Get default validation options
    ///
    /// It's important that these defaults match the default values in the builder and in the [`Config`] struct
    /// (there is a test for this).
    fn default() -> Self {
        Self {
            inheritdoc: true,
            inheritdoc_override: false,
            notice_or_dev: false,
            contracts: ContractRules::default(),
            interfaces: ContractRules::default(),
            libraries: ContractRules::default(),
            constructors: WithParamsRules::default_constructor(),
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

/// Create a [`ValidationOptions`] from a [`Config`]
impl From<Config> for ValidationOptions {
    fn from(value: Config) -> Self {
        Self {
            inheritdoc: value.lintspec.inheritdoc,
            inheritdoc_override: value.lintspec.inheritdoc_override,
            notice_or_dev: value.lintspec.notice_or_dev,
            contracts: value.contracts,
            interfaces: value.interfaces,
            libraries: value.libraries,
            constructors: value.constructors,
            enums: value.enums,
            errors: value.errors,
            events: value.events,
            functions: value.functions,
            modifiers: value.modifiers,
            structs: value.structs,
            variables: value.variables,
        }
    }
}

/// Create a [`ValidationOptions`] from a [`Config`] reference
impl From<&Config> for ValidationOptions {
    fn from(value: &Config) -> Self {
        Self {
            inheritdoc: value.lintspec.inheritdoc,
            inheritdoc_override: value.lintspec.inheritdoc_override,
            notice_or_dev: value.lintspec.notice_or_dev,
            contracts: value.contracts.clone(),
            interfaces: value.interfaces.clone(),
            libraries: value.libraries.clone(),
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

/// Params `NatSpec` checker.
#[derive(Debug, Clone, bon::Builder)]
pub struct CheckParams<'a> {
    /// The parsed [`NatSpec`], if any
    natspec: &'a Option<NatSpec>,
    /// The rule to apply for `@param`
    rule: Req,
    /// The list of actual params/members
    params: &'a [Identifier],
    /// The span of the source item, used for diagnostics which don't refer to a specific param
    default_span: TextRange,
}

impl CheckParams<'_> {
    /// Check a list of params to see if they are documented with a corresponding item in the [`NatSpec`], and generate
    /// a diagnostic for each missing one or if there are more than 1 entry per param.
    ///
    /// If the rule is [`Req::Forbidden`], it checks if `@param` is present in the [`NatSpec`] and generates a
    /// diagnostic if it is.
    #[must_use]
    pub fn check(&self) -> Vec<Diagnostic> {
        let mut res = Vec::new();
        self.check_into(&mut res);
        res.sort_unstable_by_key(|d| d.span.start.utf8);
        res
    }

    /// Check a list of params, appending diagnostics to the provided vector.
    ///
    /// This is more efficient than [`check`](Self::check) when collecting diagnostics into an existing vector.
    pub fn check_into(&self, out: &mut Vec<Diagnostic>) {
        match self.rule {
            Req::Ignored => {}
            Req::Required => self.check_required(out),
            Req::Forbidden => self.check_forbidden(out),
        }
    }

    /// Check params in case the rule is [`Req::Required`]
    fn check_required(&self, out: &mut Vec<Diagnostic>) {
        let Some(natspec) = self.natspec else {
            out.extend(self.missing_diags());
            return;
        };
        out.extend(self.extra_diags());
        out.extend(self.count_diags(natspec));
    }

    /// Check params in case the rule is [`Req::Forbidden`]
    fn check_forbidden(&self, out: &mut Vec<Diagnostic>) {
        if let Some(natspec) = self.natspec
            && natspec.has_param()
        {
            out.push(Diagnostic {
                span: self.default_span.clone(),
                message: "@param is forbidden".to_string(),
            });
        }
    }

    /// Generate missing param diags if `@param` is required
    fn missing_diags(&self) -> impl Iterator<Item = Diagnostic> {
        self.params.iter().filter_map(|p| {
            p.name.map(|name| {
                let name = INTERNER.resolve(name);
                Diagnostic {
                    span: p.span.clone(),
                    message: format!("@param {name} is missing"),
                }
            })
        })
    }

    /// Generate extra param diags if `@param` is required
    fn extra_diags(&self) -> impl Iterator<Item = Diagnostic> {
        // a HashMap is significantly slower than linear search here (few elements)
        self.natspec
            .as_ref()
            .map(|n| {
                n.items.iter().filter_map(|item| {
                    let NatSpecKind::Param { name } = item.kind else {
                        return None;
                    };
                    if self
                        .params
                        .iter()
                        .any(|p| matches!(p.name, Some(param_name) if param_name == name))
                    {
                        None
                    } else {
                        // the item's span is relative to the comment's start offset
                        let span_start = self.default_span.start + item.span.start;
                        let span_end = self.default_span.start + item.span.end;
                        let name = INTERNER.resolve(name);
                        Some(Diagnostic {
                            span: span_start..span_end,
                            message: format!("extra @param {name}"),
                        })
                    }
                })
            })
            .into_iter()
            .flatten()
    }

    /// Generate diagnostics for wrong number of `@param` comments (missing or more than one)
    fn count_diags(&self, natspec: &NatSpec) -> impl Iterator<Item = Diagnostic> {
        self.counts(natspec).filter_map(|(param, count)| {
            let name = param.name.map_or("unnamed_param", |n| INTERNER.resolve(n));
            match count {
                0 => Some(Diagnostic {
                    span: param.span.clone(),
                    message: format!("@param {name} is missing"),
                }),
                1 => None,
                2.. => Some(Diagnostic {
                    span: param.span.clone(),
                    message: format!("@param {name} is present more than once"),
                }),
            }
        })
    }

    /// Count how many times each parameter is documented
    fn counts(&self, natspec: &NatSpec) -> impl Iterator<Item = (&Identifier, usize)> {
        // a HashMap is significantly slower than linear search here (few elements)
        self.params.iter().map(|p: &Identifier| {
            let Some(param_name) = p.name else {
                return (p, 0);
            };
            (
                p,
                natspec
                    .items
                    .iter()
                    .filter(|n| matches!(n.kind, NatSpecKind::Param { name } if name == param_name))
                    .count(),
            )
        })
    }
}

/// Returns `NatSpec` checker.
#[derive(Debug, Clone, bon::Builder)]
pub struct CheckReturns<'a> {
    /// The parsed [`NatSpec`], if any
    natspec: &'a Option<NatSpec>,
    /// The rule to apply for `@return`
    rule: Req,
    /// The list of actual return values
    returns: &'a [Identifier],
    /// The span of the source item, used for diagnostics which don't refer to a specific return
    default_span: TextRange,
    /// Whether this is a state variable (affects diagnostic messages)
    is_var: bool,
}

impl CheckReturns<'_> {
    /// Check a list of returns to see if they are documented with a corresponding item in the [`NatSpec`], and generate
    /// a diagnostic for each missing one or if there are more than 1 entry per param.
    ///
    /// If the rule is [`Req::Forbidden`], it checks if `@return` is present in the [`NatSpec`] and generates a
    /// diagnostic if it is.
    #[must_use]
    pub fn check(&self) -> Vec<Diagnostic> {
        let mut res = Vec::new();
        self.check_into(&mut res);
        res
    }

    /// Check a list of returns, appending diagnostics to the provided vector.
    ///
    /// This is more efficient than [`check`](Self::check) when collecting diagnostics into an existing vector.
    pub fn check_into(&self, out: &mut Vec<Diagnostic>) {
        match self.rule {
            Req::Ignored => {}
            Req::Required => self.check_required(out),
            Req::Forbidden => self.check_forbidden(out),
        }
    }

    /// Check returns in case the rule is [`Req::Required`]
    fn check_required(&self, out: &mut Vec<Diagnostic>) {
        let Some(natspec) = self.natspec else {
            out.extend(self.missing_diags());
            return;
        };
        out.extend(self.return_diags(natspec));
        out.extend(self.extra_unnamed_diags(natspec));
    }

    /// Check returns in case the rule is [`Req::Forbidden`]
    fn check_forbidden(&self, out: &mut Vec<Diagnostic>) {
        if let Some(natspec) = self.natspec
            && natspec.has_return()
        {
            out.push(Diagnostic {
                span: self.default_span.clone(),
                message: "@return is forbidden".to_string(),
            });
        }
    }

    /// Generate missing return diags if `@return` is required and there's no natspec
    fn missing_diags(&self) -> impl Iterator<Item = Diagnostic> {
        self.returns.iter().enumerate().map(|(idx, r)| {
            let message = if let Some(name) = r.name {
                let name = INTERNER.resolve(name);
                format!("@return {name} is missing")
            } else if self.is_var {
                "@return is missing".to_string()
            } else {
                format!("@return missing for unnamed return #{}", idx + 1)
            };
            Diagnostic {
                span: r.span.clone(),
                message,
            }
        })
    }

    /// Check a named return's `NatSpec` count
    fn named_count_diag(natspec: &NatSpec, ret: &Identifier, name: &str) -> Option<Diagnostic> {
        match natspec.count_return(ret) {
            0 => Some(Diagnostic {
                span: ret.span.clone(),
                message: format!("@return {name} is missing"),
            }),
            1 => None,
            2.. => Some(Diagnostic {
                span: ret.span.clone(),
                message: format!("@return {name} is present more than once"),
            }),
        }
    }

    /// Check an unnamed return's `NatSpec`
    fn unnamed_diag(
        &self,
        returns_count: usize,
        idx: usize,
        ret: &Identifier,
    ) -> Option<Diagnostic> {
        if idx + 1 > returns_count {
            let message = if self.is_var {
                "@return is missing".to_string()
            } else {
                format!("@return missing for unnamed return #{}", idx + 1)
            };
            Some(Diagnostic {
                span: ret.span.clone(),
                message,
            })
        } else {
            None
        }
    }

    /// Generate diagnostics for all returns (both named and unnamed) in order
    fn return_diags(&self, natspec: &NatSpec) -> impl Iterator<Item = Diagnostic> {
        let returns_count = natspec.count_all_returns();
        self.returns
            .iter()
            .enumerate()
            .filter_map(move |(idx, ret)| {
                if let Some(name) = ret.name {
                    // Handle named returns
                    Self::named_count_diag(natspec, ret, INTERNER.resolve(name))
                } else {
                    // Handle unnamed returns
                    self.unnamed_diag(returns_count, idx, ret)
                }
            })
    }

    /// Generate diagnostic for too many unnamed returns in natspec
    fn extra_unnamed_diags(&self, natspec: &NatSpec) -> impl Iterator<Item = Diagnostic> {
        let unnamed_returns = self.returns.iter().filter(|r| r.name.is_none()).count();
        if natspec.count_unnamed_returns() > unnamed_returns {
            Some(Diagnostic {
                span: self
                    .returns
                    .last()
                    .cloned()
                    .map_or(self.default_span.clone(), |r| r.span),
                message: "too many unnamed returns".to_string(),
            })
        } else {
            None
        }
        .into_iter()
    }
}

/// Notice `NatSpec` checker.
#[derive(Debug, Clone, bon::Builder)]
pub struct CheckNotice<'a> {
    /// The parsed [`NatSpec`], if any
    natspec: &'a Option<NatSpec>,
    /// The rule to apply for `@notice`
    rule: Req,
    /// The span of the source item
    span: &'a TextRange,
}

impl CheckNotice<'_> {
    /// Check if the `@notice` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and generate a
    /// diagnostic if it doesn't.
    #[must_use]
    pub fn check(&self) -> Option<Diagnostic> {
        match self.rule {
            Req::Ignored => None,
            Req::Required => self.check_required(),
            Req::Forbidden => self.check_forbidden(),
        }
    }

    /// Check `@notice`, appending a diagnostic to the provided vector if needed.
    ///
    /// This is more efficient than [`check`](Self::check) when collecting diagnostics into an existing vector.
    pub fn check_into(&self, out: &mut Vec<Diagnostic>) {
        if let Some(diag) = self.check() {
            out.push(diag);
        }
    }

    /// Check notice in case the rule is [`Req::Required`]
    fn check_required(&self) -> Option<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_notice()
        {
            None
        } else {
            Some(Diagnostic {
                span: self.span.clone(),
                message: "@notice is missing".to_string(),
            })
        }
    }

    /// Check notice in case the rule is [`Req::Forbidden`]
    fn check_forbidden(&self) -> Option<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_notice()
        {
            Some(Diagnostic {
                span: self.span.clone(),
                message: "@notice is forbidden".to_string(),
            })
        } else {
            None
        }
    }
}

/// Dev `NatSpec` checker.
#[derive(Debug, Clone, bon::Builder)]
pub struct CheckDev<'a> {
    /// The parsed [`NatSpec`], if any
    natspec: &'a Option<NatSpec>,
    /// The rule to apply for `@dev`
    rule: Req,
    /// The span of the source item
    span: &'a TextRange,
}

impl CheckDev<'_> {
    /// Check if the `@dev` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and generate a
    /// diagnostic if it doesn't.
    #[must_use]
    pub fn check(&self) -> Option<Diagnostic> {
        match self.rule {
            Req::Ignored => None,
            Req::Required => self.check_required(),
            Req::Forbidden => self.check_forbidden(),
        }
    }

    /// Check `@dev`, appending a diagnostic to the provided vector if needed.
    ///
    /// This is more efficient than [`check`](Self::check) when collecting diagnostics into an existing vector.
    pub fn check_into(&self, out: &mut Vec<Diagnostic>) {
        if let Some(diag) = self.check() {
            out.push(diag);
        }
    }

    /// Check dev in case the rule is [`Req::Required`]
    fn check_required(&self) -> Option<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_dev()
        {
            None
        } else {
            Some(Diagnostic {
                span: self.span.clone(),
                message: "@dev is missing".to_string(),
            })
        }
    }

    /// Check dev in case the rule is [`Req::Forbidden`]
    fn check_forbidden(&self) -> Option<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_dev()
        {
            Some(Diagnostic {
                span: self.span.clone(),
                message: "@dev is forbidden".to_string(),
            })
        } else {
            None
        }
    }
}

/// Title `NatSpec` checker.
#[derive(Debug, Clone, bon::Builder)]
pub struct CheckTitle<'a> {
    /// The parsed [`NatSpec`], if any
    natspec: &'a Option<NatSpec>,
    /// The rule to apply for `@title`
    rule: Req,
    /// The span of the source item
    span: &'a TextRange,
}

impl CheckTitle<'_> {
    /// Check if the `@title` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and generate a
    /// diagnostic if it doesn't.
    #[must_use]
    pub fn check(&self) -> Option<Diagnostic> {
        match self.rule {
            Req::Ignored => None,
            Req::Required => self.check_required(),
            Req::Forbidden => self.check_forbidden(),
        }
    }

    /// Check `@title`, appending a diagnostic to the provided vector if needed.
    ///
    /// This is more efficient than [`check`](Self::check) when collecting diagnostics into an existing vector.
    pub fn check_into(&self, out: &mut Vec<Diagnostic>) {
        if let Some(diag) = self.check() {
            out.push(diag);
        }
    }

    /// Check title in case the rule is [`Req::Required`]
    fn check_required(&self) -> Option<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_title()
        {
            None
        } else {
            Some(Diagnostic {
                span: self.span.clone(),
                message: "@title is missing".to_string(),
            })
        }
    }

    /// Check title in case the rule is [`Req::Forbidden`]
    fn check_forbidden(&self) -> Option<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_title()
        {
            Some(Diagnostic {
                span: self.span.clone(),
                message: "@title is forbidden".to_string(),
            })
        } else {
            None
        }
    }
}

/// Author `NatSpec` checker.
#[derive(Debug, Clone, bon::Builder)]
pub struct CheckAuthor<'a> {
    /// The parsed [`NatSpec`], if any
    natspec: &'a Option<NatSpec>,
    /// The rule to apply for `@author`
    rule: Req,
    /// The span of the source item
    span: &'a TextRange,
}

impl CheckAuthor<'_> {
    /// Check if the `@author` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and generate a
    /// diagnostic if it doesn't.
    #[must_use]
    pub fn check(&self) -> Option<Diagnostic> {
        match self.rule {
            Req::Ignored => None,
            Req::Required => self.check_required(),
            Req::Forbidden => self.check_forbidden(),
        }
    }

    /// Check `@author`, appending a diagnostic to the provided vector if needed.
    ///
    /// This is more efficient than [`check`](Self::check) when collecting diagnostics into an existing vector.
    pub fn check_into(&self, out: &mut Vec<Diagnostic>) {
        if let Some(diag) = self.check() {
            out.push(diag);
        }
    }

    /// Check author in case the rule is [`Req::Required`]
    fn check_required(&self) -> Option<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_author()
        {
            None
        } else {
            Some(Diagnostic {
                span: self.span.clone(),
                message: "@author is missing".to_string(),
            })
        }
    }

    /// Check author in case the rule is [`Req::Forbidden`]
    fn check_forbidden(&self) -> Option<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_author()
        {
            Some(Diagnostic {
                span: self.span.clone(),
                message: "@author is forbidden".to_string(),
            })
        } else {
            None
        }
    }
}

/// Notice and Dev `NatSpec` checker.
#[derive(Debug, Clone, bon::Builder)]
pub struct CheckNoticeAndDev<'a> {
    /// The parsed [`NatSpec`], if any
    natspec: &'a Option<NatSpec>,
    /// The rule to apply for `@notice`
    notice_rule: Req,
    /// The rule to apply for `@dev`
    dev_rule: Req,
    /// Whether to enforce either `@notice` or `@dev` if either or both are required
    notice_or_dev: bool,
    /// The span of the source item
    span: &'a TextRange,
}

impl CheckNoticeAndDev<'_> {
    /// Check if the `@notice` or `@dev` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and
    /// generate diagnostics if they don't.
    ///
    /// This method honors the `notice_or_dev` option. If this option is enabled and one or both are required, it will
    /// check if either `@notice` or `@dev` is present in the `NatSpec`.
    ///
    /// It will generate a diagnostic if neither is present. If either is forbidden, or the `notice_or_dev` option is
    /// disabled, it will check the `@notice` and `@dev` separately according to their respective rules.
    #[must_use]
    pub fn check(&self) -> Vec<Diagnostic> {
        let mut res = Vec::new();
        self.check_into(&mut res);
        res
    }

    /// Check `@notice` and `@dev`, appending diagnostics to the provided vector.
    ///
    /// This is more efficient than [`check`](Self::check) when collecting diagnostics into an existing vector.
    pub fn check_into(&self, out: &mut Vec<Diagnostic>) {
        match (self.notice_or_dev, self.notice_rule, self.dev_rule) {
            (true, Req::Required, Req::Ignored | Req::Required)
            | (true, Req::Ignored, Req::Required) => self.check_notice_or_dev_into(out),
            (true, Req::Forbidden, _) | (true, _, Req::Forbidden) | (false, _, _) => {
                self.check_separately(out);
            }
            (true, Req::Ignored, Req::Ignored) => {}
        }
    }

    /// Check that either `@notice` or `@dev` is present
    fn check_notice_or_dev_into(&self, out: &mut Vec<Diagnostic>) {
        if let Some(natspec) = self.natspec
            && (natspec.has_notice() || natspec.has_dev())
        {
            // OK
        } else {
            out.push(Diagnostic {
                span: self.span.clone(),
                message: "@notice or @dev is missing".to_string(),
            });
        }
    }

    /// Check `@notice` and `@dev` separately according to their respective rules
    fn check_separately(&self, out: &mut Vec<Diagnostic>) {
        CheckNotice::builder()
            .natspec(self.natspec)
            .rule(self.notice_rule)
            .span(self.span)
            .build()
            .check_into(out);
        CheckDev::builder()
            .natspec(self.natspec)
            .rule(self.dev_rule)
            .span(self.span)
            .build()
            .check_into(out);
    }
}

#[cfg(test)]
mod tests {
    use similar_asserts::assert_eq;

    use crate::config::{BaseConfig, FunctionRules, NoticeDevRules};

    use super::*;

    #[test]
    fn test_validation_options_default() {
        assert_eq!(
            ValidationOptions::default(),
            ValidationOptions::builder().build()
        );

        let default_config = Config::default();
        let options = ValidationOptions::from(&default_config);
        assert_eq!(ValidationOptions::default(), options);
    }

    #[test]
    fn test_validation_options_conversion() {
        let config = Config::builder().build();
        let options = ValidationOptions::from(&config);
        assert_eq!(config.lintspec.inheritdoc, options.inheritdoc);
        assert_eq!(config.lintspec.notice_or_dev, options.notice_or_dev);
        assert_eq!(config.contracts, options.contracts);
        assert_eq!(config.interfaces, options.interfaces);
        assert_eq!(config.libraries, options.libraries);
        assert_eq!(config.constructors, options.constructors);
        assert_eq!(config.enums, options.enums);
        assert_eq!(config.errors, options.errors);
        assert_eq!(config.events, options.events);
        assert_eq!(config.functions, options.functions);
        assert_eq!(config.modifiers, options.modifiers);
        assert_eq!(config.structs, options.structs);
        assert_eq!(config.variables, options.variables);

        let config = Config::builder()
            .lintspec(
                BaseConfig::builder()
                    .inheritdoc(false)
                    .notice_or_dev(true)
                    .build(),
            )
            .contracts(
                ContractRules::builder()
                    .title(Req::Required)
                    .author(Req::Required)
                    .dev(Req::Required)
                    .notice(Req::Forbidden)
                    .build(),
            )
            .interfaces(
                ContractRules::builder()
                    .title(Req::Ignored)
                    .author(Req::Forbidden)
                    .dev(Req::Forbidden)
                    .notice(Req::Required)
                    .build(),
            )
            .libraries(
                ContractRules::builder()
                    .title(Req::Forbidden)
                    .author(Req::Ignored)
                    .dev(Req::Required)
                    .notice(Req::Ignored)
                    .build(),
            )
            .constructors(WithParamsRules::builder().dev(Req::Required).build())
            .enums(WithParamsRules::builder().param(Req::Required).build())
            .errors(WithParamsRules::builder().notice(Req::Forbidden).build())
            .events(WithParamsRules::builder().param(Req::Forbidden).build())
            .functions(
                FunctionConfig::builder()
                    .private(FunctionRules::builder().dev(Req::Required).build())
                    .build(),
            )
            .modifiers(WithParamsRules::builder().dev(Req::Forbidden).build())
            .structs(WithParamsRules::builder().notice(Req::Ignored).build())
            .variables(
                VariableConfig::builder()
                    .private(NoticeDevRules::builder().dev(Req::Required).build())
                    .build(),
            )
            .build();
        let options = ValidationOptions::from(&config);
        assert_eq!(config.lintspec.inheritdoc, options.inheritdoc);
        assert_eq!(config.lintspec.notice_or_dev, options.notice_or_dev);
        assert_eq!(config.contracts, options.contracts);
        assert_eq!(config.interfaces, options.interfaces);
        assert_eq!(config.libraries, options.libraries);
        assert_eq!(config.constructors, options.constructors);
        assert_eq!(config.enums, options.enums);
        assert_eq!(config.errors, options.errors);
        assert_eq!(config.events, options.events);
        assert_eq!(config.functions, options.functions);
        assert_eq!(config.modifiers, options.modifiers);
        assert_eq!(config.structs, options.structs);
        assert_eq!(config.variables, options.variables);
    }
}
