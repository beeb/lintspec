use anyhow::Result;

use lintspec::{config::read_config, utils::detect_solidity_version};

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load .env file if present

    let config = read_config()?;
    println!("{config:?}");

    let version = detect_solidity_version("pragma solidity >=0.8.4 <0.8.26 || 0.7.0;").unwrap();
    println!("{version:?}");

    Ok(())
}
