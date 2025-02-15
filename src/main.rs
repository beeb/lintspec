use anyhow::{bail, Result};

use lintspec::{config::read_config, files::find_sol_files};

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load .env file if present

    let config = read_config()?;
    if config.paths.is_empty() {
        bail!("no path provided, nothing to analyze");
    }
    let paths = find_sol_files(&config.paths);
    println!("{paths:#?}");

    Ok(())
}
