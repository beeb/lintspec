#![cfg(feature = "cli")]
use std::env;

use clap::{CommandFactory as _, Parser as _};
use clap_complete::{generate, generate_to};

use lintspec::cli::{Args, Commands, RunResult, read_config, run, write_default_config};

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(any(feature = "slang", feature = "solar")))]
    compile_error!("no parser enabled, please enable feature `slang` or `solar`.");

    dotenvy::dotenv().ok(); // load .env file if present

    // parse config from CLI args, environment variables and the `.lintspec.toml` file.
    let args = Args::parse();
    match &args.command {
        Some(Commands::Init) => {
            let path = write_default_config()?;
            println!("Default config was written to {}", path.display());
            println!("Exiting");
            return Ok(());
        }
        Some(Commands::Completions { shell, out }) => {
            let mut cli = Args::command();
            if let Some(out) = out {
                generate_to(*shell, &mut cli, env!("CARGO_PKG_NAME"), out)?;
            } else {
                generate(
                    *shell,
                    &mut cli,
                    env!("CARGO_PKG_NAME"),
                    &mut std::io::stdout(),
                );
            }
            return Ok(());
        }
        None => {}
    }

    let config = read_config(args)?;

    match run(&config)? {
        RunResult::NoDiagnostics => Ok(()),
        RunResult::SomeDiagnostics => std::process::exit(1), // indicate that there were lint diagnostics
    }
}
