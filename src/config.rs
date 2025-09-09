//! Tool configuration parsing and validation
use std::{env, fs, path::PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use derive_more::IsVariant;
use figment::{
    Figment, Metadata, Profile, Provider,
    providers::{Env, Format as _, Toml},
    value::{Dict, Map},
};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use crate::definitions::ItemType;

#[cfg(not(feature = "solar"))]
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(feature = "solar")]
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-solar");

/// Macro to implement the rule overrides from the CLI
macro_rules! cli_rule_override {
    ($config:expr, $items:expr, param, $req:expr) => {
        for item in $items {
            match item {
                ItemType::Constructor => $config.constructors.param = $req,
                ItemType::Enum => $config.enums.param = $req,
                ItemType::Error => $config.errors.param = $req,
                ItemType::Event => $config.events.param = $req,
                ItemType::PrivateFunction => $config.functions.private.param = $req,
                ItemType::InternalFunction => $config.functions.internal.param = $req,
                ItemType::PublicFunction => $config.functions.public.param = $req,
                ItemType::ExternalFunction => $config.functions.external.param = $req,
                ItemType::Modifier => $config.modifiers.param = $req,
                ItemType::Struct => $config.structs.param = $req,
                _ => {}
            }
        }
    };
    ($config:expr, $items:expr, return, $req:expr) => {
        for item in $items {
            match item {
                ItemType::PrivateFunction => $config.functions.private.returns = $req,
                ItemType::InternalFunction => $config.functions.internal.returns = $req,
                ItemType::PublicFunction => $config.functions.public.returns = $req,
                ItemType::ExternalFunction => $config.functions.external.returns = $req,
                ItemType::PublicVariable => $config.variables.public.returns = $req,
                _ => {}
            }
        }
    };
    ($config:expr, $items:expr, $tag:ident, $req:expr) => {
        for item in $items {
            match item {
                ItemType::Constructor => $config.constructors.$tag = $req,
                ItemType::Enum => $config.enums.$tag = $req,
                ItemType::Error => $config.errors.$tag = $req,
                ItemType::Event => $config.events.$tag = $req,
                ItemType::PrivateFunction => $config.functions.private.$tag = $req,
                ItemType::InternalFunction => $config.functions.internal.$tag = $req,
                ItemType::PublicFunction => $config.functions.public.$tag = $req,
                ItemType::ExternalFunction => $config.functions.external.$tag = $req,
                ItemType::Modifier => $config.modifiers.$tag = $req,
                ItemType::Struct => $config.structs.$tag = $req,
                ItemType::PrivateVariable => $config.variables.private.$tag = $req,
                ItemType::InternalVariable => $config.variables.internal.$tag = $req,
                ItemType::PublicVariable => $config.variables.public.$tag = $req,
                ItemType::ParsingError => {}
            }
        }
    };
}

/// The requirement for a specific tag in the natspec comment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, IsVariant)]
#[serde(rename_all = "lowercase")]
pub enum Req {
    /// The tag is ignored, can be present or not
    #[default]
    Ignored,

    /// The tag is required, must be present
    Required,

    /// The tag is forbidden, must not be present
    Forbidden,
}

impl Req {
    /// Helper function to check if the tag is required or ignored (not forbidden)
    #[must_use]
    pub fn is_required_or_ignored(&self) -> bool {
        match self {
            Req::Required | Req::Ignored => true,
            Req::Forbidden => false,
        }
    }

    /// Helper function to check if the tag is forbidden or ignored (not required)
    #[must_use]
    pub fn is_forbidden_or_ignored(&self) -> bool {
        match self {
            Req::Forbidden | Req::Ignored => true,
            Req::Required => false,
        }
    }
}

/// Validation rules for a function natspec comment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct FunctionRules {
    /// Requirement for the `@notice` tag
    #[builder(default = Req::Required)]
    pub notice: Req,

    /// Requirement for the `@dev` tag
    #[builder(default)]
    pub dev: Req,

    /// Requirement for the `@param` tags
    #[builder(default = Req::Required)]
    pub param: Req,

    /// Requirement for the `@return` tags
    #[serde(rename = "return")]
    #[builder(default = Req::Required)]
    pub returns: Req,
}

impl Default for FunctionRules {
    fn default() -> Self {
        Self {
            notice: Req::Required,
            dev: Req::default(),
            param: Req::Required,
            returns: Req::Required,
        }
    }
}

/// Validation rules for each function visibility (private, internal, public, external)
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct FunctionConfig {
    /// Rules for private functions
    #[builder(default)]
    pub private: FunctionRules,

    /// Rules for internal functions
    #[builder(default)]
    pub internal: FunctionRules,

    /// Rules for public functions
    #[builder(default)]
    pub public: FunctionRules,

    /// Rules for external functions
    #[builder(default)]
    pub external: FunctionRules,
}

/// Validation rules for items which have return values but no params (public state variables)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct WithReturnsRules {
    /// Requirement for the `@notice` tag
    #[builder(default = Req::Required)]
    pub notice: Req,

    /// Requirement for the `@dev` tag
    #[builder(default)]
    pub dev: Req,

    /// Requirement for the `@param` tags
    #[serde(rename = "return")]
    #[builder(default = Req::Required)]
    pub returns: Req,
}

impl Default for WithReturnsRules {
    fn default() -> Self {
        Self {
            notice: Req::Required,
            dev: Req::default(),
            returns: Req::Required,
        }
    }
}

/// Validation rules for items which have no return values and no params (private and internal state variables)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct NoticeDevRules {
    #[builder(default = Req::Required)]
    pub notice: Req,
    #[builder(default)]
    pub dev: Req,
}

impl Default for NoticeDevRules {
    fn default() -> Self {
        Self {
            notice: Req::Required,
            dev: Req::default(),
        }
    }
}

/// Validation rules for each state variable visibility (private, internal, public)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct VariableConfig {
    #[builder(default)]
    pub private: NoticeDevRules,
    #[builder(default)]
    pub internal: NoticeDevRules,
    #[builder(default)]
    pub public: WithReturnsRules,
}

/// Validation rules for items which have params but no returns (constructor, enum, error, event, modifier, struct)
///
/// The default value does not enforce that `@param` is present, because it's not part of the official spec for enums
/// and structs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct WithParamsRules {
    #[builder(default = Req::Required)]
    pub notice: Req,
    #[builder(default)]
    pub dev: Req,
    #[builder(default)]
    pub param: Req,
}

impl Default for WithParamsRules {
    fn default() -> Self {
        Self {
            notice: Req::Required,
            dev: Req::default(),
            param: Req::default(),
        }
    }
}

impl WithParamsRules {
    /// Helper function to get a set of rules which enforces params (default for error, event, modifier)
    #[must_use]
    pub fn required() -> Self {
        Self {
            param: Req::Required,
            ..Default::default()
        }
    }

    /// Helper function to get a default set of rules for constructors (`@notice` is not enforced, but `@param` is)
    #[must_use]
    pub fn default_constructor() -> Self {
        Self {
            notice: Req::Ignored,
            param: Req::Required,
            ..Default::default()
        }
    }
}

/// General config for the tool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[skip_serializing_none]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)]
pub struct BaseConfig {
    /// Paths to files and folders to analyze
    #[builder(default)]
    pub paths: Vec<PathBuf>,

    /// Paths to files and folders to exclude
    #[builder(default)]
    pub exclude: Vec<PathBuf>,

    /// Enforce that all public and external items have `@inheritdoc`
    #[builder(default = true)]
    pub inheritdoc: bool,

    /// Enforce that internal functions and modifiers which are `override` have `@inheritdoc`
    #[builder(default = false)]
    pub inheritdoc_override: bool,

    /// Do not distinguish between `@notice` and `@dev` when considering "required" validation rules
    #[builder(default)]
    pub notice_or_dev: bool,

    /// Skip the detection of the Solidity version and use the latest version supported by [`slang_solidity`]
    #[builder(default)]
    pub skip_version_detection: bool,
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            paths: Vec::default(),
            exclude: Vec::default(),
            inheritdoc: true,
            inheritdoc_override: false,
            notice_or_dev: false,
            skip_version_detection: false,
        }
    }
}

/// Output config for the tool
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, bon::Builder)]
#[skip_serializing_none]
#[non_exhaustive]
pub struct OutputConfig {
    /// Path to a file to write the output to (instead of stderr)
    pub out: Option<PathBuf>,

    /// Output diagnostics in JSON format
    #[builder(default)]
    pub json: bool,

    /// Compact output (minified JSON or compact text representation)
    #[builder(default)]
    pub compact: bool,

    /// Sort the results by file path
    #[builder(default)]
    pub sort: bool,
}

/// The parsed and validated config for the tool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[skip_serializing_none]
#[non_exhaustive]
pub struct Config {
    /// General config for the tool
    #[builder(default)]
    pub lintspec: BaseConfig,

    /// Output config for the tool
    #[builder(default)]
    pub output: OutputConfig,

    /// Validation rules for constructors
    #[serde(rename = "constructor")]
    #[builder(default = WithParamsRules::default_constructor())]
    pub constructors: WithParamsRules,

    /// Validation rules for enums
    #[serde(rename = "enum")]
    #[builder(default)]
    pub enums: WithParamsRules,

    /// Validation rules for errors
    #[serde(rename = "error")]
    #[builder(default = WithParamsRules::required())]
    pub errors: WithParamsRules,

    /// Validation rules for events
    #[serde(rename = "event")]
    #[builder(default = WithParamsRules::required())]
    pub events: WithParamsRules,

    /// Validation rules for functions
    #[serde(rename = "function")]
    #[builder(default)]
    pub functions: FunctionConfig,

    /// Validation rules for modifiers
    #[serde(rename = "modifier")]
    #[builder(default = WithParamsRules::required())]
    pub modifiers: WithParamsRules,

    /// Validation rules for structs
    #[serde(rename = "struct")]
    #[builder(default)]
    pub structs: WithParamsRules,

    /// Validation rules for state variables
    #[serde(rename = "variable")]
    #[builder(default)]
    pub variables: VariableConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lintspec: BaseConfig::default(),
            output: OutputConfig::default(),
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

impl Config {
    #[allow(clippy::result_large_err)]
    pub fn from(provider: impl Provider) -> Result<Config, figment::Error> {
        Figment::from(provider).extract()
    }

    /// Create a Figment which reads the config from the default file and environment variables
    #[must_use]
    pub fn figment(config_path: Option<PathBuf>) -> Figment {
        Figment::from(Config::default())
            .admerge(Toml::file(config_path.unwrap_or(".lintspec.toml".into())))
            .admerge(Env::prefixed("LS_").split("_").map(|k| {
                // special case for parameters with an underscore in the name
                match k.as_str() {
                    "LINTSPEC.NOTICE.OR.DEV" => "LINTSPEC.NOTICE_OR_DEV".into(),
                    "LINTSPEC.SKIP.VERSION.DETECTION" => "LINTSPEC.SKIP_VERSION_DETECTION".into(),
                    _ => k.into(),
                }
            }))
    }
}

/// Implement [`Provider`] for composability
impl Provider for Config {
    fn metadata(&self) -> figment::Metadata {
        Metadata::named("LintSpec Config")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        figment::providers::Serialized::defaults(Config::default()).data()
    }
}

#[derive(Subcommand, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Commands {
    /// Create a `.lintspec.toml` config file with default values
    Init,
}

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[skip_serializing_none]
#[command(version = VERSION, about, long_about = None)]
#[non_exhaustive]
pub struct Args {
    /// One or more paths to files and folders to analyze
    #[arg(name = "PATH", value_hint = clap::ValueHint::AnyPath)]
    pub paths: Vec<PathBuf>,

    /// Path to a file or folder to exclude (can be used more than once)
    ///
    /// To exclude paths based on a pattern, use a `.nsignore` file (same syntax as `.gitignore`).
    #[arg(short, long, value_hint = clap::ValueHint::AnyPath)]
    pub exclude: Vec<PathBuf>,

    /// Optional path to a TOML config file
    ///
    /// If unspecified, the default path is `./.lintspec.toml`.
    #[arg(long, value_hint = clap::ValueHint::FilePath)]
    pub config: Option<PathBuf>,

    /// Write output to a file instead of stderr
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    pub out: Option<PathBuf>,

    /// Enforce that all public and external items have `@inheritdoc`
    ///
    /// Can be set with `--inheritdoc` (means true), `--inheritdoc=true` or `--inheritdoc=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub inheritdoc: Option<bool>,

    /// Enforce that internal functions and modifiers which override a parent have `@inheritdoc`
    ///
    /// Can be set with `--inheritdoc-override` (means true), `--inheritdoc-override=true` or
    /// `--inheritdoc-override=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub inheritdoc_override: Option<bool>,

    /// Do not distinguish between `@notice` and `@dev` when considering "required" validation rules.
    ///
    /// If either `dev = "required"` or `notice = "required"` (or both), this allows to enforce that at least one
    /// `@dev` or one `@notice` is present, but not dictate which one it should be.
    /// This flag has no effect if either `dev = "forbidden"` or `notice = "forbidden"`.
    ///
    /// Can be set with `--notice-or-dev` (means true), `--notice-or-dev=true` or `--notice-or-dev=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub notice_or_dev: Option<bool>,

    /// Skip the detection of the Solidity version from pragma statements and use the latest supported version.
    ///
    /// This is useful to speed up parsing slightly, or if the Solidity version is newer than the latest version
    /// supported by the parser.
    ///
    /// Can be set with `--skip-version-detection` (means true), `--skip-version-detection=true` or `--skip-version-detection=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub skip_version_detection: Option<bool>,

    /// Ignore `@notice` for these items (can be used more than once)
    #[arg(long)]
    pub notice_ignored: Vec<ItemType>,

    /// Enforce `@notice` for these items (can be used more than once)
    ///
    /// This takes precedence over `--notice-ignored`.
    #[arg(long)]
    pub notice_required: Vec<ItemType>,

    /// Forbid `@notice` for these items (can be used more than once)
    ///
    /// This takes precedence over `--notice-required`.
    #[arg(long)]
    pub notice_forbidden: Vec<ItemType>,

    /// Ignore `@dev` for these items (can be used more than once)
    #[arg(long)]
    pub dev_ignored: Vec<ItemType>,

    /// Enforce `@dev` for these items (can be used more than once)
    ///
    /// This takes precedence over `--dev-ignored`.
    #[arg(long)]
    pub dev_required: Vec<ItemType>,

    /// Forbid `@dev` for these items (can be used more than once)
    ///
    /// This takes precedence over `--dev-required`.
    #[arg(long)]
    pub dev_forbidden: Vec<ItemType>,

    /// Ignore `@param` for these items (can be used more than once)
    ///
    /// Note that this setting is ignored for `*-variable`.
    #[arg(long)]
    pub param_ignored: Vec<ItemType>,

    /// Enforce `@param` for these items (can be used more than once)
    ///
    /// Note that this setting is ignored for `*-variable`.
    /// This takes precedence over `--param-ignored`.
    #[arg(long)]
    pub param_required: Vec<ItemType>,

    /// Forbid `@param` for these items (can be used more than once)
    ///
    /// Note that this setting is ignored for `*-variable`.
    /// This takes precedence over `--param-required`.
    #[arg(long)]
    pub param_forbidden: Vec<ItemType>,

    /// Ignore `@return` for these items (can be used more than once)
    ///
    /// Note that this setting is only applicable for `*-function`, `public-variable`.
    #[arg(long)]
    pub return_ignored: Vec<ItemType>,

    /// Enforce `@return` for these items (can be used more than once)
    ///
    /// Note that this setting is only applicable for `*-function`, `public-variable`.
    /// This takes precedence over `--return-ignored`.
    #[arg(long)]
    pub return_required: Vec<ItemType>,

    /// Forbid `@return` for these items (can be used more than once)
    ///
    /// Note that this setting is only applicable for `*-function`, `public-variable`.
    /// This takes precedence over `--return-required`.
    #[arg(long)]
    pub return_forbidden: Vec<ItemType>,

    /// Output diagnostics in JSON format
    ///
    /// Can be set with `--json` (means true), `--json=true` or `--json=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub json: Option<bool>,

    /// Compact output
    ///
    /// If combined with `--json`, the output is minified.
    ///
    /// Can be set with `--compact` (means true), `--compact=true` or `--compact=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub compact: Option<bool>,

    /// Sort the results by file path
    ///
    /// Can be set with `--sort` (means true), `--sort=true` or `--sort=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub sort: Option<bool>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Read the configuration from config file, environment variables and parsed CLI arguments (passed as argument)
pub fn read_config(args: Args) -> Result<Config> {
    let config_path = args
        .config
        .or_else(|| env::var("LS_CONFIG_PATH").ok().map(Into::into));
    let mut config: Config = Config::figment(config_path).extract()?;
    // paths
    config.lintspec.paths.extend(args.paths);
    config.lintspec.exclude.extend(args.exclude);
    // output
    if let Some(out) = args.out {
        config.output.out = Some(out);
    }
    if let Some(json) = args.json {
        config.output.json = json;
    }
    if let Some(compact) = args.compact {
        config.output.compact = compact;
    }
    if let Some(sort) = args.sort {
        config.output.sort = sort;
    }
    // parser
    if let Some(skip_version_detection) = args.skip_version_detection {
        config.lintspec.skip_version_detection = skip_version_detection;
    }
    // natspec config
    if let Some(inheritdoc) = args.inheritdoc {
        config.lintspec.inheritdoc = inheritdoc;
    }
    if let Some(inheritdoc_override) = args.inheritdoc_override {
        config.lintspec.inheritdoc_override = inheritdoc_override;
    }
    if let Some(notice_or_dev) = args.notice_or_dev {
        config.lintspec.notice_or_dev = notice_or_dev;
    }

    cli_rule_override!(config, args.notice_ignored, notice, Req::Ignored);
    cli_rule_override!(config, args.notice_required, notice, Req::Required);
    cli_rule_override!(config, args.notice_forbidden, notice, Req::Forbidden);
    cli_rule_override!(config, args.dev_ignored, dev, Req::Ignored);
    cli_rule_override!(config, args.dev_required, dev, Req::Required);
    cli_rule_override!(config, args.dev_forbidden, dev, Req::Forbidden);
    cli_rule_override!(config, args.param_ignored, param, Req::Ignored);
    cli_rule_override!(config, args.param_required, param, Req::Required);
    cli_rule_override!(config, args.param_forbidden, param, Req::Forbidden);
    cli_rule_override!(config, args.return_ignored, return, Req::Ignored);
    cli_rule_override!(config, args.return_required, return, Req::Required);
    cli_rule_override!(config, args.return_forbidden, return, Req::Forbidden);

    Ok(config)
}

/// Write the default configuration to a `.lintspec.toml` file in the current directory.
///
/// If a file already exists with the same name, it gets renamed to `.lintspec.bck.toml` before writing the default
/// config.
pub fn write_default_config() -> Result<PathBuf> {
    let config = Config::default();
    let path = PathBuf::from(".lintspec.toml");
    if path.exists() {
        fs::rename(&path, ".lintspec.bck.toml")?;
        println!("Existing `.lintspec.toml` file was renamed to `.lintpsec.bck.toml`");
    }
    fs::write(&path, toml::to_string(&config)?)?;
    Ok(dunce::canonicalize(path)?)
}

#[cfg(test)]
mod tests {
    use similar_asserts::assert_eq;

    use super::*;

    #[test]
    fn test_default_builder() {
        assert_eq!(FunctionRules::default(), FunctionRules::builder().build());
        assert_eq!(FunctionConfig::default(), FunctionConfig::builder().build());
        assert_eq!(
            WithReturnsRules::default(),
            WithReturnsRules::builder().build()
        );
        assert_eq!(NoticeDevRules::default(), NoticeDevRules::builder().build());
        assert_eq!(VariableConfig::default(), VariableConfig::builder().build());
        assert_eq!(
            WithParamsRules::default(),
            WithParamsRules::builder().build()
        );
        assert_eq!(BaseConfig::default(), BaseConfig::builder().build());
        assert_eq!(OutputConfig::default(), OutputConfig::builder().build());
        assert_eq!(Config::default(), Config::builder().build());
    }
}
