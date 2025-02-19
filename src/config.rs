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
    /// To exclude paths based on a pattern, use a `.lsignore` file (same syntax as `.gitignore`).
    #[arg(short, long, value_hint = clap::ValueHint::AnyPath)]
    pub exclude: Vec<PathBuf>,

    /// Write output to a file instead of stderr
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    pub out: Option<PathBuf>,

    /// Enforce that all public and external items have `@inheritdoc`
    ///
    /// Functions which override a parent function also must have `@inheritdoc`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub inheritdoc: Option<bool>,

    /// Enforce that constructors have NatSpec
    #[arg(long)]
    pub constructor: bool,

    /// Enforce that enums have `@param` for each variant
    #[arg(long)]
    pub enum_params: bool,

    /// Output diagnostics in JSON format
    #[arg(long)]
    pub json: bool,

    /// Compact output
    ///
    /// If combined with `--json`, the output is minified.
    #[arg(long)]
    pub compact: bool,
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
            constructor: value.constructor,
            enum_params: value.enum_params,
            json: value.json,
            compact: value.compact,
        }
    }
}

pub fn read_config() -> Result<Config> {
    let temp: Args = Figment::new()
        .admerge(Toml::file(".lintspec.toml"))
        .admerge(Env::prefixed("LINTSPEC_"))
        .admerge(Serialized::defaults(Args::parse())) // FIXME: None should not override any value from the TOML
        .extract()?;
    Ok(temp.into())
}
