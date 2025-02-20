use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use figment::{
    providers::{Env, Format as _, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub paths: Vec<PathBuf>,
    pub exclude: Vec<PathBuf>,
    pub out: Option<PathBuf>,
    pub inheritdoc: bool,
    pub constructor: bool,
    pub enum_params: bool,
    pub json: bool,
    pub compact: bool,
}

impl From<Args> for Config {
    fn from(value: Args) -> Self {
        Self {
            paths: value.paths,
            exclude: value.exclude,
            out: value.out,
            inheritdoc: value.inheritdoc.unwrap_or(true),
            constructor: value.constructor.unwrap_or_default(),
            enum_params: value.enum_params.unwrap_or_default(),
            json: value.json.unwrap_or_default(),
            compact: value.compact.unwrap_or_default(),
        }
    }
}

pub fn read_config() -> Result<Config> {
    let args = Args::parse();
    let mut temp: Args = Figment::new()
        .admerge(Serialized::defaults(Args {
            paths: args.paths,
            exclude: args.exclude,
            out: None,
            inheritdoc: None,
            constructor: None,
            enum_params: None,
            json: None,
            compact: None,
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
    if let Some(enum_params) = args.enum_params {
        temp.enum_params = Some(enum_params);
    }
    if let Some(json) = args.json {
        temp.json = Some(json);
    }
    if let Some(compact) = args.compact {
        temp.compact = Some(compact);
    }
    Ok(temp.into())
}
