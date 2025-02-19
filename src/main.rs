use std::{env, fs::File};

use anyhow::{bail, Result};
use lintspec::{
    config::read_config, definitions::ValidationOptions, error::Error, files::find_sol_files,
    lint::lint, print_reports,
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
        .filter_map(|p| {
            lint(p, &options, config.compact || config.json)
                .map_err(Into::into)
                .transpose()
        })
        .collect::<Result<Vec<_>>>()?;

    let mut output_file: Box<dyn std::io::Write> = match config.out {
        Some(path) => Box::new(File::open(&path).map_err(|err| Error::IOError {
            path: path.clone(),
            err,
        })?),
        None => match diagnostics.is_empty() {
            true => Box::new(std::io::stdout()),
            false => Box::new(std::io::stderr()),
        },
    };

    if diagnostics.is_empty() {
        if config.json {
            write!(&mut output_file, "[]")?;
        } else {
            writeln!(&mut output_file, "No issue found")?;
        }
        return Ok(());
    }
    if config.json {
        if config.compact {
            write!(&mut output_file, "{}", serde_json::to_string(&diagnostics)?)?;
        } else {
            write!(
                &mut output_file,
                "{}",
                serde_json::to_string_pretty(&diagnostics)?
            )?;
        }
    } else {
        let cwd = dunce::canonicalize(env::current_dir()?)?;
        for file_diags in diagnostics {
            print_reports(&mut output_file, &cwd, file_diags, config.compact)?;
        }
    }
    std::process::exit(1);
}
