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
    /// Path(s) to files and folders to analyze
    #[arg(value_hint = clap::ValueHint::AnyPath)]
    pub path: Vec<PathBuf>,
}

pub fn read_config() -> Result<Config> {
    Figment::new()
        .adjoin(Serialized::defaults(Config::parse()))
        .adjoin(Env::prefixed("LINTSPEC_"))
        .adjoin(Toml::file(".lintspec.toml"))
        .extract()
        .map_err(Into::into)
}
