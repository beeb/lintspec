use anyhow::{bail, Result};

use lintspec::{config::read_config, files::find_sol_files};

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load .env file if present

    let config = read_config()?;

    let paths = find_sol_files(&config.paths);
    if paths.is_empty() {
        bail!("no Solidity file found, nothing to analyze");
    }

    Ok(())
}
