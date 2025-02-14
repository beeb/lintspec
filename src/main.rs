use anyhow::Result;

use lintspec::config::read_config;

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load .env file if present

    let config = read_config()?;

    println!("{config:?}");

    Ok(())
}
