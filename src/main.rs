use anyhow::{bail, Result};
use lintspec::{
    config::read_config, definitions::ValidationOptions, files::find_sol_files, lint::lint,
};
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator};

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load .env file if present

    let config = read_config()?;

    let paths = find_sol_files(&config.paths, &config.exclude)?;
    if paths.is_empty() {
        bail!("no Solidity file found, nothing to analyze");
    }

    let options = ValidationOptions {
        constructor: config.constructor,
        enum_params: config.enum_params,
    };
    let mut diagnostics = paths
        .par_iter()
        .map(|p| lint(p, &options).map_err(Into::into))
        .collect::<Result<Vec<_>>>()?;
    diagnostics.retain(|p| p.is_some());

    if config.json {
        print!("{}", serde_json::to_string_pretty(&diagnostics)?);
    } else {
        println!("{diagnostics:#?}");
    }
    if diagnostics.is_empty() {
        return Ok(());
    }
    if !config.json {
        println!("Some files contain errors");
    }
    std::process::exit(1);
}
