use anyhow::{bail, Result};
use lintspec::{config::read_config, files::find_sol_files, lint::lint};
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator};

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load .env file if present

    let config = read_config()?;

    let paths = find_sol_files(&config.paths);
    if paths.is_empty() {
        bail!("no Solidity file found, nothing to analyze");
    }

    let diagnostics = paths
        .par_iter()
        .map(|p| lint(p).map_err(Into::into))
        .collect::<Result<Vec<_>>>()?;

    println!("{diagnostics:?}");

    Ok(())
}
