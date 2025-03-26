use std::{env, fs::File};

use anyhow::{bail, Result};
use clap::Parser as _;
use lintspec::{
    config::{read_config, write_default_config, Args, Commands},
    error::Error,
    files::find_sol_files,
    lint::{lint, ValidationOptions},
    parser::{slang::SlangParser, solar::SolarParser},
    print_reports,
};
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator};

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load .env file if present

    // parse config from CLI args, environment variables and the `.lintspec.toml` file.
    let args = Args::parse();
    if let Some(Commands::Init) = args.command {
        let path = write_default_config()?;
        println!("Default config was written to {path:?}");
        println!("Exiting");
        return Ok(());
    }

    let config = read_config(args)?;

    // identify Solidity files to parse
    let paths = find_sol_files(
        &config.lintspec.paths,
        &config.lintspec.exclude,
        config.output.sort,
    )?;
    if paths.is_empty() {
        bail!("no Solidity file found, nothing to analyze");
    }

    // lint all the requested Solidity files
    let options: ValidationOptions = (&config).into();
    let diagnostics = paths
        .par_iter()
        .filter_map(|p| {
            let parser = if config.lintspec.use_solar {
                lint::<SolarParser>
            } else {
                lint::<SlangParser>
            };

            parser(p, &options, !config.output.compact && !config.output.json)
                .map_err(Into::into)
                .transpose()
        })
        .collect::<Result<Vec<_>>>()?;

    // check if we should output to file or to stderr/stdout
    let mut output_file: Box<dyn std::io::Write> = match config.output.out {
        Some(path) => {
            let _ = miette::set_hook(Box::new(|_| {
                Box::new(
                    miette::MietteHandlerOpts::new()
                        .terminal_links(false)
                        .unicode(false)
                        .color(false)
                        .build(),
                )
            }));
            Box::new(
                File::options()
                    .truncate(true)
                    .create(true)
                    .write(true)
                    .open(&path)
                    .map_err(|err| Error::IOError {
                        path: path.clone(),
                        err,
                    })?,
            )
        }
        None => {
            if diagnostics.is_empty() {
                Box::new(std::io::stdout())
            } else {
                Box::new(std::io::stderr())
            }
        }
    };

    // no issue was found
    if diagnostics.is_empty() {
        if config.output.json {
            writeln!(&mut output_file, "[]")?;
        } else {
            writeln!(&mut output_file, "No issue found")?;
        }
        return Ok(());
    }

    // some issues were found, output according to the desired format (json/text, pretty/compact)
    if config.output.json {
        if config.output.compact {
            writeln!(&mut output_file, "{}", serde_json::to_string(&diagnostics)?)?;
        } else {
            writeln!(
                &mut output_file,
                "{}",
                serde_json::to_string_pretty(&diagnostics)?
            )?;
        }
    } else {
        let cwd = dunce::canonicalize(env::current_dir()?)?;
        for file_diags in diagnostics {
            print_reports(&mut output_file, &cwd, file_diags, config.output.compact)?;
        }
    }
    std::process::exit(1); // indicate that there were diagnostics (errors)
}
