#![cfg(feature = "cli")]
use std::{collections::HashMap, env, fs::File};

use anyhow::{Result, bail};
use clap::Parser as _;
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator};

use lintspec::{
    cli::{Args, Commands, print_reports, read_config, write_default_config},
    error::Error,
    files::find_sol_files,
    lint::{ValidationOptions, lint},
    parser::Parse as _,
};

#[cfg(feature = "slang")]
use lintspec::parser::slang::SlangParser;

#[cfg(feature = "solar")]
use lintspec::parser::solar::SolarParser;

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    #[cfg(not(any(feature = "slang", feature = "solar")))]
    compile_error!("no parser enabled, please enable feature `slang` or `solar`.");

    dotenvy::dotenv().ok(); // load .env file if present

    // parse config from CLI args, environment variables and the `.lintspec.toml` file.
    let args = Args::parse();
    if let Some(Commands::Init) = args.command {
        let path = write_default_config()?;
        println!("Default config was written to {}", path.display());
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

    #[allow(unused_variables)]
    #[cfg(feature = "slang")]
    let parser = SlangParser::builder()
        .skip_version_detection(config.lintspec.skip_version_detection)
        .build();

    #[cfg(feature = "solar")]
    let parser = SolarParser::default();

    let diagnostics = paths
        .par_iter()
        .filter_map(|p| {
            lint(
                parser.clone(),
                p,
                &options,
                !config.output.compact && !config.output.json,
            )
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
        let mut contents = if cfg!(any(feature = "slang", feature = "solar")) {
            // all other clones have been dropped
            parser.get_sources()?
        } else {
            HashMap::default()
        };
        for file_diags in diagnostics {
            let Some(source) = contents.remove(&file_diags.document_id) else {
                continue;
            };
            print_reports(
                &mut output_file,
                &cwd,
                file_diags,
                source,
                config.output.compact,
            )?;
        }
    }
    std::process::exit(1); // indicate that there were diagnostics (errors)
}
