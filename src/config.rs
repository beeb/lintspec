use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use figment::{
    providers::{Env, Serialized},
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
        .merge(Serialized::defaults(Config::parse()))
        .merge(Env::prefixed("LINTSPEC_"))
        .extract()
        .map_err(Into::into)
}
