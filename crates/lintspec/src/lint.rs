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
        fn inner(
            this: &ItemDiagnostics,
            f: &mut impl io::Write,
            path: &Path,
            root_dir: &Path,
        ) -> std::result::Result<(), io::Error> {
            let source_name = match path.strip_prefix(root_dir) {
                Ok(relative_path) => relative_path.to_string_lossy(),
                Err(_) => path.to_string_lossy(),
            };
            writeln!(f, "{source_name}:{}", this.span.start)?;
            if let Some(parent) = &this.parent {
                writeln!(f, "{} {}.{}", this.item_type, parent, this.name)?;
            } else {
                writeln!(f, "{} {}", this.item_type, this.name)?;
            }
            for diag in &this.diags {
                writeln!(f, "  {}", diag.message)?;
            }
            writeln!(f)?;
            Ok(())
        }
        inner(self, f, path.as_ref(), root_dir.as_ref())
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

/// Params NatSpec checker.
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
        let mut res = match self.rule {
            Req::Ignored => return Vec::new(),
            Req::Required => self.check_required(),
            Req::Forbidden => self.check_forbidden(),
        };
        res.sort_unstable_by_key(|d| d.span.start.utf8);
        res
    }

    /// Check params in case the rule is [`Req::Required`]
    fn check_required(&self) -> Vec<Diagnostic> {
        let Some(natspec) = self.natspec else {
            return self.missing_diags().collect();
        };
        self.extra_diags()
            .chain(self.count_diags(natspec))
            .collect()
    }

    /// Check params in case the rule is [`Req::Forbidden`]
    fn check_forbidden(&self) -> Vec<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_param()
        {
            vec![Diagnostic {
                span: self.default_span.clone(),
                message: "@param is forbidden".to_string(),
            }]
        } else {
            Vec::new()
        }
    }

    /// Generate missing param diags if `@param` is required
    fn missing_diags(&self) -> impl Iterator<Item = Diagnostic> {
        self.params.iter().filter_map(|p| {
            p.name.as_ref().map(|name| Diagnostic {
                span: p.span.clone(),
                message: format!("@param {name} is missing"),
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
                    let NatSpecKind::Param { name } = &item.kind else {
                        return None;
                    };
                    if self
                        .params
                        .iter()
                        .any(|p| matches!(p.name.as_ref(), Some(param_name) if param_name == name))
                    {
                        None
                    } else {
                        // the item's span is relative to the comment's start offset
                        let span_start = self.default_span.start + item.span.start;
                        let span_end = self.default_span.start + item.span.end;
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
            let name = param.name.as_ref().expect("param to have a name");
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
            let param_name = p.name.as_ref().expect("params to have a name");
            (
                p,
                natspec
                    .items
                    .iter()
                    .filter(
                        |n| matches!(&n.kind, NatSpecKind::Param { name } if name == param_name),
                    )
                    .count(),
            )
        })
    }
}

/// Returns NatSpec checker.
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
        match self.rule {
            Req::Ignored => Vec::new(),
            Req::Required => self.check_required(),
            Req::Forbidden => self.check_forbidden(),
        }
    }

    /// Check returns in case the rule is [`Req::Required`]
    fn check_required(&self) -> Vec<Diagnostic> {
        let Some(natspec) = self.natspec else {
            return self.missing_diags().collect();
        };
        self.return_diags(natspec)
            .chain(self.extra_unnamed_diags(natspec))
            .collect()
    }

    /// Check returns in case the rule is [`Req::Forbidden`]
    fn check_forbidden(&self) -> Vec<Diagnostic> {
        if let Some(natspec) = self.natspec
            && natspec.has_return()
        {
            vec![Diagnostic {
                span: self.default_span.clone(),
                message: "@return is forbidden".to_string(),
            }]
        } else {
            Vec::new()
        }
    }

    /// Generate missing return diags if `@return` is required and there's no natspec
    fn missing_diags(&self) -> impl Iterator<Item = Diagnostic> {
        self.returns.iter().enumerate().map(|(idx, r)| {
            let message = if let Some(name) = &r.name {
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

    /// Check a named return's NatSpec count
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

    /// Check an unnamed return's NatSpec
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
                if let Some(name) = &ret.name {
                    // Handle named returns
                    Self::named_count_diag(natspec, ret, name)
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

/// Check if the `@notice` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and generate a
/// diagnostic if it doesn't.
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

/// Check if the `@dev` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and generate a
/// diagnostic if it doesn't.
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

/// Check if the `@title` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and generate a
/// diagnostic if it doesn't.
#[must_use]
pub fn check_title(natspec: &Option<NatSpec>, rule: Req, span: TextRange) -> Option<Diagnostic> {
    // add default `NatSpec` to avoid duplicate match arms for None vs Some with no notice
    match (rule, natspec, &NatSpec::default()) {
        (Req::Required, Some(natspec), _) | (Req::Required, None, natspec)
            if !natspec.has_title() =>
        {
            Some(Diagnostic {
                span,
                message: "@title is missing".to_string(),
            })
        }
        (Req::Forbidden, Some(natspec), _) if natspec.has_title() => Some(Diagnostic {
            span,
            message: "@title is forbidden".to_string(),
        }),
        _ => None,
    }
}

/// Check if the `@author` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and generate a
/// diagnostic if it doesn't.
#[must_use]
pub fn check_author(natspec: &Option<NatSpec>, rule: Req, span: TextRange) -> Option<Diagnostic> {
    // add default `NatSpec` to avoid duplicate match arms for None vs Some with no notice
    match (rule, natspec, &NatSpec::default()) {
        (Req::Required, Some(natspec), _) | (Req::Required, None, natspec)
            if !natspec.has_author() =>
        {
            Some(Diagnostic {
                span,
                message: "@author is missing".to_string(),
            })
        }
        (Req::Forbidden, Some(natspec), _) if natspec.has_author() => Some(Diagnostic {
            span,
            message: "@author is forbidden".to_string(),
        }),
        _ => None,
    }
}

/// Check if the `@notice` or `@dev` presence matches the requirements (`Req::Required` or `Req::Forbidden`) and
/// generate diagnostics if they don't.
///
/// This helper function should be used to honor the `notice_or_dev` option in the [`Config`]. If this option is
/// enabled and one or both are required, it will check if either `@notice` or `@dev` is present in the `NatSpec`.
///
/// It will generate a diagnostic if neither is present. If either is forbidden, or the `notice_or_dev` option is
/// disabled, it will check the `@notice` and `@dev` separately according to their respective rules.
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
