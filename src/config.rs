//! Tool configuration parsing and validation
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use derive_more::IsVariant;
use figment::{
    providers::{Env, Format as _, Toml},
    value::{Dict, Map},
    Figment, Metadata, Profile, Provider,
};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum, Default, IsVariant,
)]
#[serde(rename_all = "lowercase")]
pub enum Enforcement {
    Require,
    Disallow,
    #[default]
    Ignore,
}

impl Enforcement {
    #[must_use]
    pub fn is_require_or_ignore(&self) -> bool {
        match self {
            Enforcement::Require | Enforcement::Ignore => true,
            Enforcement::Disallow => false,
        }
    }

    #[must_use]
    pub fn is_disallow_or_ignore(&self) -> bool {
        match self {
            Enforcement::Disallow | Enforcement::Ignore => true,
            Enforcement::Require => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct FunctionEnforcement {
    pub notice: Enforcement,
    pub dev: Enforcement,
    pub param: Enforcement,
    #[serde(rename = "return")]
    pub returns: Enforcement,
}

impl FunctionEnforcement {
    #[must_use]
    pub fn required() -> Self {
        Self {
            notice: Enforcement::default(),
            dev: Enforcement::default(),
            param: Enforcement::Require,
            returns: Enforcement::Require,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FunctionConfig {
    pub private: FunctionEnforcement,
    pub internal: FunctionEnforcement,
    pub public: FunctionEnforcement,
    pub external: FunctionEnforcement,
}

impl Default for FunctionConfig {
    fn default() -> Self {
        Self {
            private: FunctionEnforcement::default(),
            internal: FunctionEnforcement::default(),
            public: FunctionEnforcement::required(),
            external: FunctionEnforcement::required(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct WithReturnsEnforcement {
    pub notice: Enforcement,
    pub dev: Enforcement,
    #[serde(rename = "return")]
    pub returns: Enforcement,
}

impl WithReturnsEnforcement {
    #[must_use]
    pub fn required() -> Self {
        Self {
            notice: Enforcement::default(),
            dev: Enforcement::default(),
            returns: Enforcement::Require,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VariableConfig {
    pub private: WithReturnsEnforcement,
    pub internal: WithReturnsEnforcement,
    pub public: WithReturnsEnforcement,
}

impl Default for VariableConfig {
    fn default() -> Self {
        Self {
            private: WithReturnsEnforcement::default(),
            internal: WithReturnsEnforcement::default(),
            public: WithReturnsEnforcement::required(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct WithParamsEnforcement {
    pub notice: Enforcement,
    pub dev: Enforcement,
    pub param: Enforcement,
}

impl WithParamsEnforcement {
    #[must_use]
    pub fn required() -> Self {
        Self {
            notice: Enforcement::default(),
            dev: Enforcement::default(),
            param: Enforcement::Require,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[skip_serializing_none]
#[non_exhaustive]
pub struct LintSpecConfig {
    pub paths: Vec<PathBuf>,
    pub exclude: Vec<PathBuf>,
    pub inheritdoc: bool,
}

impl Default for LintSpecConfig {
    fn default() -> Self {
        Self {
            paths: Vec::default(),
            exclude: Vec::default(),
            inheritdoc: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[skip_serializing_none]
#[non_exhaustive]
pub struct OutputConfig {
    pub out: Option<PathBuf>,
    pub json: bool,
    pub compact: bool,
    pub sort: bool,
}

/// The parsed and validated config for the tool
#[derive(Debug, Clone, Serialize, Deserialize)]
#[skip_serializing_none]
#[non_exhaustive]
pub struct Config {
    pub lintspec: LintSpecConfig,

    pub output: OutputConfig,

    #[serde(rename = "constructor")]
    pub constructors: WithParamsEnforcement,

    #[serde(rename = "enum")]
    pub enums: WithParamsEnforcement,

    #[serde(rename = "error")]
    pub errors: WithParamsEnforcement,

    #[serde(rename = "event")]
    pub events: WithParamsEnforcement,

    #[serde(rename = "function")]
    pub functions: FunctionConfig,

    #[serde(rename = "modifier")]
    pub modifiers: WithParamsEnforcement,

    #[serde(rename = "struct")]
    pub structs: WithParamsEnforcement,

    #[serde(rename = "variable")]
    pub variables: VariableConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lintspec: LintSpecConfig::default(),
            output: OutputConfig::default(),
            constructors: WithParamsEnforcement::default(),
            enums: WithParamsEnforcement::default(),
            errors: WithParamsEnforcement::required(),
            events: WithParamsEnforcement::required(),
            functions: FunctionConfig::default(),
            modifiers: WithParamsEnforcement::default(),
            structs: WithParamsEnforcement::default(),
            variables: VariableConfig::default(),
        }
    }
}

impl Config {
    pub fn from(provider: impl Provider) -> Result<Config, figment::Error> {
        Figment::from(provider).extract()
    }

    #[must_use]
    pub fn figment() -> Figment {
        Figment::from(Config::default())
            .admerge(Toml::file(".lintspec.toml"))
            .admerge(Env::prefixed("LINTSPEC_"))
    }
}

impl Provider for Config {
    fn metadata(&self) -> figment::Metadata {
        Metadata::named("LintSpec Config")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        figment::providers::Serialized::defaults(Config::default()).data()
    }
}

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[skip_serializing_none]
#[command(version, about, long_about = None)]
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

    /// Write output to a file instead of stderr
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    pub out: Option<PathBuf>,

    /// Enforce that all public and external items have `@inheritdoc`
    ///
    /// Functions which override a parent function also must have `@inheritdoc`.
    ///
    /// Can be set with `--inheritdoc` (means true), `--inheritdoc true` or `--inheritdoc false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub inheritdoc: Option<bool>,

    /// Enforce that constructors have NatSpec
    ///
    /// Can be set with `--constructor` (means true), `--constructor true` or `--constructor false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub constructor: Option<bool>,

    /// Enforce that structs have `@param` for each member
    ///
    /// Can be set with `--struct_params` (means true), `--struct_params true` or `--struct_params false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub struct_params: Option<bool>,

    /// Enforce that enums have `@param` for each variant
    ///
    /// Can be set with `--enum_params` (means true), `--enum_params true` or `--enum_params false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub enum_params: Option<bool>,

    /// Output diagnostics in JSON format
    ///
    /// Can be set with `--json` (means true), `--json true` or `--json false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub json: Option<bool>,

    /// Compact output
    ///
    /// If combined with `--json`, the output is minified.
    ///
    /// Can be set with `--compact` (means true), `--compact true` or `--compact false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub compact: Option<bool>,

    /// Sort the results by file path
    ///
    /// Can be set with `--sort` (means true), `--sort true` or `--sort false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub sort: Option<bool>,
}

/// Read the configuration from config file, environment variables and CLI arguments
pub fn read_config() -> Result<Config> {
    let args = Args::parse();
    let mut config: Config = Config::figment().extract()?;
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
    // natspec config
    if let Some(inheritdoc) = args.inheritdoc {
        config.lintspec.inheritdoc = inheritdoc;
    }
    if let Some(constructor) = args.constructor {
        config.constructors.param = if constructor {
            Enforcement::Require
        } else {
            Enforcement::Ignore
        };
    }
    if let Some(struct_params) = args.struct_params {
        config.structs.param = if struct_params {
            Enforcement::Require
        } else {
            Enforcement::Ignore
        };
    }
    if let Some(enum_params) = args.enum_params {
        config.enums.param = if enum_params {
            Enforcement::Require
        } else {
            Enforcement::Ignore
        };
    }
    Ok(config)
}
