//! Tool configuration parsing and validation
use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
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
pub enum Req {
    #[default]
    Ignored,
    Required,
    Forbidden,
}

impl Req {
    #[must_use]
    pub fn is_required_or_ignored(&self) -> bool {
        match self {
            Req::Required | Req::Ignored => true,
            Req::Forbidden => false,
        }
    }

    #[must_use]
    pub fn is_forbidden_or_ignored(&self) -> bool {
        match self {
            Req::Forbidden | Req::Ignored => true,
            Req::Required => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, bon::Builder)]
#[non_exhaustive]
pub struct FunctionRules {
    #[builder(default)]
    pub notice: Req,
    #[builder(default)]
    pub dev: Req,
    #[builder(default = Req::Required)]
    pub param: Req,
    #[serde(rename = "return")]
    #[builder(default = Req::Required)]
    pub returns: Req,
}

impl FunctionRules {
    #[must_use]
    pub fn required() -> Self {
        Self {
            notice: Req::default(),
            dev: Req::default(),
            param: Req::Required,
            returns: Req::Required,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct FunctionConfig {
    #[builder(default)]
    pub private: FunctionRules,
    #[builder(default)]
    pub internal: FunctionRules,
    #[builder(default = FunctionRules::required())]
    pub public: FunctionRules,
    #[builder(default = FunctionRules::required())]
    pub external: FunctionRules,
}

impl Default for FunctionConfig {
    fn default() -> Self {
        Self {
            private: FunctionRules::default(),
            internal: FunctionRules::default(),
            public: FunctionRules::required(),
            external: FunctionRules::required(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, bon::Builder)]
#[non_exhaustive]
pub struct WithReturnsRules {
    #[builder(default)]
    pub notice: Req,
    #[builder(default)]
    pub dev: Req,
    #[serde(rename = "return")]
    #[builder(default)]
    pub returns: Req,
}

impl WithReturnsRules {
    #[must_use]
    pub fn required() -> Self {
        Self {
            notice: Req::default(),
            dev: Req::default(),
            returns: Req::Required,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, bon::Builder)]
#[non_exhaustive]
pub struct NoticeDevRules {
    #[builder(default)]
    pub notice: Req,
    #[builder(default)]
    pub dev: Req,
}

#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct VariableConfig {
    #[builder(default)]
    pub private: NoticeDevRules,
    #[builder(default)]
    pub internal: NoticeDevRules,
    #[builder(default = WithReturnsRules::required())]
    pub public: WithReturnsRules,
}

impl Default for VariableConfig {
    fn default() -> Self {
        Self {
            private: NoticeDevRules::default(),
            internal: NoticeDevRules::default(),
            public: WithReturnsRules::required(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, bon::Builder)]
#[non_exhaustive]
pub struct WithParamsRules {
    #[builder(default)]
    pub notice: Req,
    #[builder(default)]
    pub dev: Req,
    #[builder(default)]
    pub param: Req,
}

impl WithParamsRules {
    #[must_use]
    pub fn required() -> Self {
        Self {
            notice: Req::default(),
            dev: Req::default(),
            param: Req::Required,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[skip_serializing_none]
#[non_exhaustive]
pub struct BaseConfig {
    #[builder(default)]
    pub paths: Vec<PathBuf>,
    #[builder(default)]
    pub exclude: Vec<PathBuf>,
    #[builder(default = true)]
    pub inheritdoc: bool,
    #[builder(default)]
    pub notice_or_dev: bool,
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            paths: Vec::default(),
            exclude: Vec::default(),
            inheritdoc: true,
            notice_or_dev: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, bon::Builder)]
#[skip_serializing_none]
#[non_exhaustive]
pub struct OutputConfig {
    pub out: Option<PathBuf>,
    #[builder(default)]
    pub json: bool,
    #[builder(default)]
    pub compact: bool,
    #[builder(default)]
    pub sort: bool,
}

/// The parsed and validated config for the tool
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[skip_serializing_none]
#[non_exhaustive]
pub struct Config {
    #[builder(default)]
    pub lintspec: BaseConfig,

    #[builder(default)]
    pub output: OutputConfig,

    #[serde(rename = "constructor")]
    #[builder(default)]
    pub constructors: WithParamsRules,

    #[serde(rename = "enum")]
    #[builder(default)]
    pub enums: WithParamsRules,

    #[serde(rename = "error")]
    #[builder(default = WithParamsRules::required())]
    pub errors: WithParamsRules,

    #[serde(rename = "event")]
    #[builder(default = WithParamsRules::required())]
    pub events: WithParamsRules,

    #[serde(rename = "function")]
    #[builder(default)]
    pub functions: FunctionConfig,

    #[serde(rename = "modifier")]
    #[builder(default = WithParamsRules::required())]
    pub modifiers: WithParamsRules,

    #[serde(rename = "struct")]
    #[builder(default)]
    pub structs: WithParamsRules,

    #[serde(rename = "variable")]
    #[builder(default)]
    pub variables: VariableConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lintspec: BaseConfig::default(),
            output: OutputConfig::default(),
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

#[derive(Subcommand, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Commands {
    /// Create a `.lintspec.toml` config file with default values
    Init,
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
    /// Can be set with `--inheritdoc` (means true), `--inheritdoc=true` or `--inheritdoc=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub inheritdoc: Option<bool>,

    /// Do not distinguish between `@notice` and `@dev` when considering "required" validation rules.
    ///
    /// If either `dev = "required"` or `notice = "required"` (or both), this allows to enforce that at least one
    /// `@dev` or one `@notice` is present, but not dictate which one it should be.
    /// This flag has no effect if either `dev = "forbidden"` or `notice = "forbidden"`.
    ///
    /// Can be set with `--notice-or-dev` (means true), `--notice-or-dev=true` or `--notice-or-dev=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub notice_or_dev: Option<bool>,

    /// Enforce that constructors have `@param` if applicable
    ///
    /// Can be set with `--constructor` (means true), `--constructor=true` or `--constructor=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub constructor: Option<bool>,

    /// Enforce that structs have `@param` for each member
    ///
    /// Can be set with `--struct_params` (means true), `--struct_params=true` or `--struct_params=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub struct_params: Option<bool>,

    /// Enforce that enums have `@param` for each variant
    ///
    /// Can be set with `--enum_params` (means true), `--enum_params=true` or `--enum_params=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub enum_params: Option<bool>,

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

/// Read the configuration from config file, environment variables and CLI arguments
pub fn read_config(args: Args) -> Result<Config> {
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
    if let Some(notice_or_dev) = args.notice_or_dev {
        config.lintspec.notice_or_dev = notice_or_dev;
    }
    if let Some(constructor) = args.constructor {
        config.constructors.param = if constructor {
            Req::Required
        } else {
            Req::Ignored
        };
    }
    if let Some(struct_params) = args.struct_params {
        config.structs.param = if struct_params {
            Req::Required
        } else {
            Req::Ignored
        };
    }
    if let Some(enum_params) = args.enum_params {
        config.enums.param = if enum_params {
            Req::Required
        } else {
            Req::Ignored
        };
    }
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
