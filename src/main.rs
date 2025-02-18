use std::env;

use anyhow::{bail, Result};
use lintspec::{
    config::read_config, definitions::ValidationOptions, files::find_sol_files, lint::lint,
    print_reports,
};
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator};

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load .env file if present

    let config = read_config()?;

    let paths = find_sol_files(&config.paths, &config.exclude)?;
    if paths.is_empty() {
        bail!("no Solidity file found, nothing to analyze");
    }

    let options: ValidationOptions = (&config).into();
    let diagnostics = paths
        .par_iter()
        .filter_map(|p| lint(p, &options).map_err(Into::into).transpose())
        .collect::<Result<Vec<_>>>()?;

    if diagnostics.is_empty() {
        if config.json {
            print!("[]");
        } else {
            println!("No issue found");
        }
        return Ok(());
    }
    if config.json {
        eprint!("{}", serde_json::to_string_pretty(&diagnostics)?);
    } else {
        let cwd = dunce::canonicalize(env::current_dir()?)?;
        for file_diags in diagnostics {
            print_reports(&cwd, file_diags);
        }
    }
    std::process::exit(1);
}
