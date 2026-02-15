use std::{
    collections::HashMap,
    env,
    error::Error,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use miette::{LabeledSpan, MietteDiagnostic, NamedSource};
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator};

use lintspec_core::{
    config::{Config, Req},
    definitions::{ContractType, ItemType},
    files::find_sol_files,
    lint::{FileDiagnostics, ItemDiagnostics, ValidationOptions, lint},
    parser::Parse as _,
};

#[cfg(feature = "slang")]
use lintspec_core::parser::slang::SlangParser;

#[cfg(feature = "solar")]
use lintspec_core::parser::solar::SolarParser;

#[cfg(not(feature = "slang"))]
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(feature = "slang")]
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-slang");

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Create a `.lintspec.toml` config file with default values
    Init,

    /// Generate shell completion scripts
    Completions {
        /// The flavor of shell for which to generate the completion script
        #[arg(short, long)]
        shell: Shell,

        /// The output directory for the file, prints to `stdout` if omitted
        #[arg(short, long, value_hint = clap::ValueHint::DirPath)]
        out: Option<PathBuf>,
    },
}

#[derive(Parser, Debug, Clone)]
#[command(version = VERSION, about, long_about = None)]
#[non_exhaustive]
pub struct Args {
    /// One or more paths to files and folders to analyze
    #[arg(name = "PATH", value_hint = clap::ValueHint::AnyPath)]
    pub paths: Vec<PathBuf>,

    /// Path to a file or folder to exclude (can be used more than once)
    ///
    /// To exclude paths based on a pattern, use a `.nsignore` file (same syntax as `.gitignore`).
    #[arg(short, long, value_hint = clap::ValueHint::AnyPath)]
    pub exclude: Vec<PathBuf>,

    /// Optional path to a TOML config file
    ///
    /// If unspecified, the default path is `./.lintspec.toml`.
    #[arg(long, value_hint = clap::ValueHint::FilePath)]
    pub config: Option<PathBuf>,

    /// Write output to a file instead of stderr
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    pub out: Option<PathBuf>,

    /// Enforce that all public and external items have `@inheritdoc`
    ///
    /// Can be set with `--inheritdoc` (means true), `--inheritdoc=true` or `--inheritdoc=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub inheritdoc: Option<bool>,

    /// Enforce that internal functions and modifiers which override a parent have `@inheritdoc`
    ///
    /// Can be set with `--inheritdoc-override` (means true), `--inheritdoc-override=true` or
    /// `--inheritdoc-override=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub inheritdoc_override: Option<bool>,

    /// Do not distinguish between `@notice` and `@dev` when considering "required" validation rules.
    ///
    /// If either `dev = "required"` or `notice = "required"` (or both), this allows to enforce that at least one
    /// `@dev` or one `@notice` is present, but not dictate which one it should be.
    /// This flag has no effect if either `dev = "forbidden"` or `notice = "forbidden"`.
    ///
    /// Can be set with `--notice-or-dev` (means true), `--notice-or-dev=true` or `--notice-or-dev=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub notice_or_dev: Option<bool>,

    /// Skip the detection of the Solidity version from pragma statements and use the latest supported version.
    ///
    /// This is useful to speed up parsing slightly, or if the Solidity version is newer than the latest version
    /// supported by the parser.
    ///
    /// Can be set with `--skip-version-detection` (means true), `--skip-version-detection=true` or `--skip-version-detection=false`.
    #[cfg(feature = "slang")]
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub skip_version_detection: Option<bool>,

    /// Ignore `@title` for these items (can be used more than once)
    #[arg(long, value_enum)]
    pub title_ignored: Vec<ContractType>,

    /// Enforce `@title` for these items (can be used more than once)
    ///
    /// This takes precedence over `--title-ignored`.
    #[arg(long, value_enum)]
    pub title_required: Vec<ContractType>,

    /// Forbid `@title` for these items (can be used more than once)
    ///
    /// This takes precedence over `--title-required`.
    #[arg(long, value_enum)]
    pub title_forbidden: Vec<ContractType>,

    /// Ignore `@author` for these items (can be used more than once)
    #[arg(long, value_enum)]
    pub author_ignored: Vec<ContractType>,

    /// Enforce `@author` for these items (can be used more than once)
    ///
    /// This takes precedence over `--author-ignored`.
    #[arg(long, value_enum)]
    pub author_required: Vec<ContractType>,

    /// Forbid `@author` for these items (can be used more than once)
    ///
    /// This takes precedence over `--author-required`.
    #[arg(long, value_enum)]
    pub author_forbidden: Vec<ContractType>,

    /// Ignore `@notice` for these items (can be used more than once)
    #[arg(long, value_enum)]
    pub notice_ignored: Vec<ItemType>,

    /// Enforce `@notice` for these items (can be used more than once)
    ///
    /// This takes precedence over `--notice-ignored`.
    #[arg(long, value_enum)]
    pub notice_required: Vec<ItemType>,

    /// Forbid `@notice` for these items (can be used more than once)
    ///
    /// This takes precedence over `--notice-required`.
    #[arg(long, value_enum)]
    pub notice_forbidden: Vec<ItemType>,

    /// Ignore `@dev` for these items (can be used more than once)
    #[arg(long, value_enum)]
    pub dev_ignored: Vec<ItemType>,

    /// Enforce `@dev` for these items (can be used more than once)
    ///
    /// This takes precedence over `--dev-ignored`.
    #[arg(long, value_enum)]
    pub dev_required: Vec<ItemType>,

    /// Forbid `@dev` for these items (can be used more than once)
    ///
    /// This takes precedence over `--dev-required`.
    #[arg(long, value_enum)]
    pub dev_forbidden: Vec<ItemType>,

    /// Ignore `@param` for these items (can be used more than once)
    ///
    /// Note that this setting is ignored for `*-variable`.
    #[arg(long, value_enum)]
    pub param_ignored: Vec<ItemType>,

    /// Enforce `@param` for these items (can be used more than once)
    ///
    /// Note that this setting is ignored for `*-variable`.
    /// This takes precedence over `--param-ignored`.
    #[arg(long, value_enum)]
    pub param_required: Vec<ItemType>,

    /// Forbid `@param` for these items (can be used more than once)
    ///
    /// Note that this setting is ignored for `*-variable`.
    /// This takes precedence over `--param-required`.
    #[arg(long, value_enum)]
    pub param_forbidden: Vec<ItemType>,

    /// Ignore `@return` for these items (can be used more than once)
    ///
    /// Note that this setting is only applicable for `*-function`, `public-variable`.
    #[arg(long, value_enum)]
    pub return_ignored: Vec<ItemType>,

    /// Enforce `@return` for these items (can be used more than once)
    ///
    /// Note that this setting is only applicable for `*-function`, `public-variable`.
    /// This takes precedence over `--return-ignored`.
    #[arg(long, value_enum)]
    pub return_required: Vec<ItemType>,

    /// Forbid `@return` for these items (can be used more than once)
    ///
    /// Note that this setting is only applicable for `*-function`, `public-variable`.
    /// This takes precedence over `--return-required`.
    #[arg(long, value_enum)]
    pub return_forbidden: Vec<ItemType>,

    /// Output diagnostics in JSON format
    ///
    /// Can be set with `--json` (means true), `--json=true` or `--json=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub json: Option<bool>,

    /// Compact output
    ///
    /// If combined with `--json`, the output is minified.
    ///
    /// Can be set with `--compact` (means true), `--compact=true` or `--compact=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub compact: Option<bool>,

    /// Sort the results by file path
    ///
    /// Can be set with `--sort` (means true), `--sort=true` or `--sort=false`.
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub sort: Option<bool>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Macro to implement the rule overrides from the CLI
macro_rules! cli_rule_override {
    ($config:expr, $items:expr, param, $req:expr) => {
        for item in $items {
            match item {
                ItemType::Constructor => $config.constructors.param = $req,
                ItemType::Enum => $config.enums.param = $req,
                ItemType::Error => $config.errors.param = $req,
                ItemType::Event => $config.events.param = $req,
                ItemType::PrivateFunction => $config.functions.private.param = $req,
                ItemType::InternalFunction => $config.functions.internal.param = $req,
                ItemType::PublicFunction => $config.functions.public.param = $req,
                ItemType::ExternalFunction => $config.functions.external.param = $req,
                ItemType::Modifier => $config.modifiers.param = $req,
                ItemType::Struct => $config.structs.param = $req,
                _ => {}
            }
        }
    };
    ($config:expr, $items:expr, return, $req:expr) => {
        for item in $items {
            match item {
                ItemType::PrivateFunction => $config.functions.private.returns = $req,
                ItemType::InternalFunction => $config.functions.internal.returns = $req,
                ItemType::PublicFunction => $config.functions.public.returns = $req,
                ItemType::ExternalFunction => $config.functions.external.returns = $req,
                ItemType::PublicVariable => $config.variables.public.returns = $req,
                _ => {}
            }
        }
    };
    ($config:expr, $items:expr, title, $req:expr) => {
        for item in $items {
            match item {
                ContractType::Contract => $config.contracts.title = $req,
                ContractType::Interface => $config.interfaces.title = $req,
                ContractType::Library => $config.libraries.title = $req,
            }
        }
    };
    ($config:expr, $items:expr, author, $req:expr) => {
        for item in $items {
            match item {
                ContractType::Contract => $config.contracts.author = $req,
                ContractType::Interface => $config.interfaces.author = $req,
                ContractType::Library => $config.libraries.author = $req,
            }
        }
    };
    ($config:expr, $items:expr, $tag:ident, $req:expr) => {
        for item in $items {
            match item {
                ItemType::Contract => $config.contracts.$tag = $req,
                ItemType::Interface => $config.interfaces.$tag = $req,
                ItemType::Library => $config.libraries.$tag = $req,
                ItemType::Constructor => $config.constructors.$tag = $req,
                ItemType::Enum => $config.enums.$tag = $req,
                ItemType::Error => $config.errors.$tag = $req,
                ItemType::Event => $config.events.$tag = $req,
                ItemType::PrivateFunction => $config.functions.private.$tag = $req,
                ItemType::InternalFunction => $config.functions.internal.$tag = $req,
                ItemType::PublicFunction => $config.functions.public.$tag = $req,
                ItemType::ExternalFunction => $config.functions.external.$tag = $req,
                ItemType::Modifier => $config.modifiers.$tag = $req,
                ItemType::Struct => $config.structs.$tag = $req,
                ItemType::PrivateVariable => $config.variables.private.$tag = $req,
                ItemType::InternalVariable => $config.variables.internal.$tag = $req,
                ItemType::PublicVariable => $config.variables.public.$tag = $req,
                ItemType::ParsingError => {}
            }
        }
    };
}

/// Read the configuration from config file, environment variables and parsed CLI arguments (passed as argument)
pub fn read_config(args: Args) -> Result<Config, Box<figment::Error>> {
    let config_path = args
        .config
        .or_else(|| env::var("LS_CONFIG_PATH").ok().map(Into::into));
    let mut config: Config = Config::figment(config_path).extract()?;
    // paths
    config.lintspec.paths.extend(args.paths);
    config.lintspec.exclude.extend(args.exclude);
    // output
    if let Some(out) = args.out {
        config.output.out = Some(out);
    }
    if let Some(json) = args.json {
        config.output.json = json;
    }
    if let Some(compact) = args.compact {
        config.output.compact = compact;
    }
    if let Some(sort) = args.sort {
        config.output.sort = sort;
    }
    // parser
    #[cfg(feature = "slang")]
    if let Some(skip_version_detection) = args.skip_version_detection {
        config.lintspec.skip_version_detection = skip_version_detection;
    }
    // natspec config
    if let Some(inheritdoc) = args.inheritdoc {
        config.lintspec.inheritdoc = inheritdoc;
    }
    if let Some(inheritdoc_override) = args.inheritdoc_override {
        config.lintspec.inheritdoc_override = inheritdoc_override;
    }
    if let Some(notice_or_dev) = args.notice_or_dev {
        config.lintspec.notice_or_dev = notice_or_dev;
    }

    cli_rule_override!(config, args.title_ignored, title, Req::Ignored);
    cli_rule_override!(config, args.title_required, title, Req::Required);
    cli_rule_override!(config, args.title_forbidden, title, Req::Forbidden);
    cli_rule_override!(config, args.author_ignored, author, Req::Ignored);
    cli_rule_override!(config, args.author_required, author, Req::Required);
    cli_rule_override!(config, args.author_forbidden, author, Req::Forbidden);
    cli_rule_override!(config, args.notice_ignored, notice, Req::Ignored);
    cli_rule_override!(config, args.notice_required, notice, Req::Required);
    cli_rule_override!(config, args.notice_forbidden, notice, Req::Forbidden);
    cli_rule_override!(config, args.dev_ignored, dev, Req::Ignored);
    cli_rule_override!(config, args.dev_required, dev, Req::Required);
    cli_rule_override!(config, args.dev_forbidden, dev, Req::Forbidden);
    cli_rule_override!(config, args.param_ignored, param, Req::Ignored);
    cli_rule_override!(config, args.param_required, param, Req::Required);
    cli_rule_override!(config, args.param_forbidden, param, Req::Forbidden);
    cli_rule_override!(config, args.return_ignored, return, Req::Ignored);
    cli_rule_override!(config, args.return_required, return, Req::Required);
    cli_rule_override!(config, args.return_forbidden, return, Req::Forbidden);

    Ok(config)
}

/// The result of running the tool
pub enum RunResult {
    NoDiagnostics,
    SomeDiagnostics,
}

/// Run lintspec
pub fn run(config: &Config) -> Result<RunResult, Box<dyn Error>> {
    // identify Solidity files to parse
    let paths = find_sol_files(
        &config.lintspec.paths,
        &config.lintspec.exclude,
        config.output.sort,
    )?;
    if paths.is_empty() {
        return Err(String::from("no Solidity file found, nothing to analyze").into());
    }

    // lint all the requested Solidity files
    let options: ValidationOptions = config.into();

    #[cfg_attr(all(feature = "slang", feature = "slang"), expect(unused_variables))]
    #[cfg(feature = "solar")]
    let parser = SolarParser::default();

    #[cfg(feature = "slang")]
    let parser = SlangParser::builder()
        .skip_version_detection(config.lintspec.skip_version_detection)
        .build();

    let diagnostics = paths
        .par_iter()
        .filter_map(|p| {
            lint(
                parser.clone(),
                p,
                &options,
                !config.output.compact && !config.output.json,
            )
            .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;

    // check if we should output to file or to stderr/stdout
    let mut output_file: Box<dyn std::io::Write> = match &config.output.out {
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
                    .open(path)
                    .map_err(|err| lintspec_core::error::ErrorKind::IOError {
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
        return Ok(RunResult::NoDiagnostics);
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
            let source = contents.remove(&file_diags.document_id).unwrap_or_default();
            print_reports(
                &mut output_file,
                &cwd,
                file_diags,
                source,
                config.output.compact,
            )?;
        }
    }
    Ok(RunResult::SomeDiagnostics)
}

/// Write the default configuration to a `.lintspec.toml` file in the current directory.
///
/// If a file already exists with the same name, it gets renamed to `.lintspec.bck.toml` before writing the default
/// config.
pub fn write_default_config() -> Result<PathBuf, Box<dyn Error>> {
    let config = Config::default();
    let path = PathBuf::from(".lintspec.toml");
    if path.exists() {
        fs::rename(&path, ".lintspec.bck.toml")?;
        println!("Existing `.lintspec.toml` file was renamed to `.lintpsec.bck.toml`");
    }
    fs::write(&path, toml::to_string(&config)?)?;
    Ok(dunce::canonicalize(&path)?)
}

/// Print the reports for a given file, either as pretty or compact text output
///
/// The root path is the current working directory used to compute relative paths if possible. If the file path is
/// not a child of the root path, then the full canonical path of the file is used instead.
/// The writer can be anything that implement [`io::Write`].
pub fn print_reports(
    f: &mut impl io::Write,
    root_path: impl AsRef<Path>,
    file_diags: FileDiagnostics,
    contents: String,
    compact: bool,
) -> Result<(), io::Error> {
    fn inner(
        f: &mut impl io::Write,
        root_path: &Path,
        file_diags: FileDiagnostics,
        contents: String,
        compact: bool,
    ) -> Result<(), io::Error> {
        if compact {
            let source_name = match file_diags.path.strip_prefix(root_path) {
                Ok(relative_path) => relative_path.to_string_lossy(),
                Err(_) => file_diags.path.to_string_lossy(),
            };
            for item_diags in file_diags.items {
                item_diags.print_compact(f, &source_name)?;
            }
        } else {
            let source_name = match file_diags.path.strip_prefix(root_path) {
                Ok(relative_path) => relative_path.to_string_lossy(),
                Err(_) => file_diags.path.to_string_lossy(),
            };
            let source = Arc::new(NamedSource::new(source_name, contents));
            for item_diags in file_diags.items {
                print_report(f, Arc::clone(&source), item_diags)?;
            }
        }
        Ok(())
    }
    inner(f, root_path.as_ref(), file_diags, contents, compact)
}

/// Print a single report related to one source item with [`miette`].
///
/// The writer can be anything that implement [`io::Write`].
fn print_report(
    f: &mut impl io::Write,
    source: Arc<NamedSource<String>>,
    item: ItemDiagnostics,
) -> Result<(), io::Error> {
    let msg = if let Some(parent) = &item.parent {
        format!("{} {}.{}", item.item_type, parent, item.name)
    } else {
        format!("{} {}", item.item_type, item.name)
    };
    let labels: Vec<_> = item
        .diags
        .into_iter()
        .map(|d| {
            LabeledSpan::new(
                Some(d.message),
                d.span.start.utf8,
                d.span.end.utf8 - d.span.start.utf8,
            )
        })
        .collect();
    let report: miette::Report = MietteDiagnostic::new(msg).with_labels(labels).into();
    write!(f, "{:?}", report.with_source_code(source))
}
