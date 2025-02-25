use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use figment::{
    providers::{Env, Format as _, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use crate::lint::ItemType;

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

    /// Enforce NatSpec on items even if they don't have params/returns/members
    /// (can be used more than once)
    ///
    /// To enforce for all types, you can use `--enforce-all`.
    #[arg(short = 'f', long, name = "TYPE")]
    pub enforce: Vec<ItemType>,

    /// Enforce NatSpec for all item types, even if they don't have params/returns/members
    ///
    /// Setting this option overrides any previously set `--enforce` arguments.
    /// Can be set with `--enforce-all` (means true), `--enforce-all true` or `--enforce-all false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub enforce_all: Option<bool>,

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

/// The parsed and validated config for the tool
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
#[builder(on(PathBuf, into))]
#[allow(clippy::struct_excessive_bools)]
pub struct Config {
    /// The paths to search for Solidity files
    pub paths: Vec<PathBuf>,

    /// Some paths to ignore while searching for Solidity files
    pub exclude: Vec<PathBuf>,

    /// The file where to output the diagnostics (if `None`, then stderr is used)
    pub out: Option<PathBuf>,

    /// Whether to enforce the use of `@inheritdoc` on external/public/overridden items
    pub inheritdoc: bool,

    /// Whether to enforce documentation of constructors
    pub constructor: bool,

    /// Whether to enforce documentation of struct members
    pub struct_params: bool,

    /// Whether to enforce documentation of enum variants
    pub enum_params: bool,

    /// Whether to enforce documentation of items which have no params/returns/members
    pub enforce: Vec<ItemType>,

    /// Output JSON diagnostics
    pub json: bool,

    /// Output compact format (minified JSON or simple text output)
    pub compact: bool,

    /// Sort diagnostics by file path
    pub sort: bool,
}

impl From<Args> for Config {
    fn from(value: Args) -> Self {
        Self {
            paths: value.paths,
            exclude: value.exclude,
            out: value.out,
            inheritdoc: value.inheritdoc.unwrap_or(true),
            constructor: value.constructor.unwrap_or_default(),
            struct_params: value.struct_params.unwrap_or_default(),
            enum_params: value.enum_params.unwrap_or_default(),
            enforce: value.enforce,
            json: value.json.unwrap_or_default(),
            compact: value.compact.unwrap_or_default(),
            sort: value.sort.unwrap_or_default(),
        }
    }
}

/// Read the configuration from config file, environment variables and CLI arguments
pub fn read_config() -> Result<Config> {
    let args = Args::parse();
    let mut temp: Args = Figment::new()
        .admerge(Serialized::defaults(Args {
            paths: args.paths,
            exclude: args.exclude,
            out: None,
            inheritdoc: None,
            constructor: None,
            struct_params: None,
            enum_params: None,
            enforce: args.enforce,
            enforce_all: None,
            json: None,
            compact: None,
            sort: None,
        }))
        .admerge(Toml::file(".lintspec.toml"))
        .admerge(Env::prefixed("LINTSPEC_"))
        .extract()?;
    if let Some(out) = args.out {
        temp.out = Some(out);
    }
    if let Some(inheritdoc) = args.inheritdoc {
        temp.inheritdoc = Some(inheritdoc);
    }
    if let Some(constructor) = args.constructor {
        temp.constructor = Some(constructor);
    }
    if let Some(struct_params) = args.struct_params {
        temp.struct_params = Some(struct_params);
    }
    if let Some(enum_params) = args.enum_params {
        temp.enum_params = Some(enum_params);
    }
    // first look if enforce_all was set in a config file or env variable
    if let Some(enforce_all) = temp.enforce_all {
        if enforce_all {
            temp.enforce = ItemType::value_variants().into();
        } else {
            temp.enforce = vec![];
        }
    }
    // then look if it was set in the CLI args
    if let Some(enforce_all) = args.enforce_all {
        if enforce_all {
            temp.enforce = ItemType::value_variants().into();
        } else {
            temp.enforce = vec![];
        }
    }
    if let Some(json) = args.json {
        temp.json = Some(json);
    }
    if let Some(compact) = args.compact {
        temp.compact = Some(compact);
    }
    if let Some(sort) = args.sort {
        temp.sort = Some(sort);
    }
    Ok(temp.into())
}
