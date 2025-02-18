use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use figment::{
    providers::{Env, Format as _, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(version, about, long_about = None)]
#[non_exhaustive]
pub struct Config {
    /// One or more paths to files and folders to analyze
    #[arg(name = "PATH", value_hint = clap::ValueHint::AnyPath)]
    pub paths: Vec<PathBuf>,

    /// Path to a file or folder to exclude (can be used more than once)
    ///
    /// To exclude paths based on a pattern, use a `.lsignore` file (same syntax as `.gitignore`).
    #[arg(short, long, value_hint = clap::ValueHint::AnyPath)]
    pub exclude: Vec<PathBuf>,

    /// Enforce that all public and external items have `@inheritdoc`
    ///
    /// Functions which override a parent function also must have `@inheritdoc`.
    #[arg(short, long, default_value_t = true)]
    pub inheritdoc: bool,

    /// Enforce that constructors have NatSpec
    #[arg(short, long, default_value_t = false)]
    pub constructor: bool,

    /// Enforce that enums have `@param` NatSpec for each variant
    #[arg(long, default_value_t = false)]
    pub enum_params: bool,

    /// Output diagnostics in JSON format
    #[arg(short, long, default_value_t = false)]
    pub json: bool,
}

pub fn read_config() -> Result<Config> {
    Figment::new()
        .adjoin(Serialized::defaults(Config::parse()))
        .adjoin(Env::prefixed("LINTSPEC_"))
        .adjoin(Toml::file(".lintspec.toml"))
        .extract()
        .map_err(Into::into)
}
