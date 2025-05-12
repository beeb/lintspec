//! The error and result types for lintspec
use std::path::PathBuf;

use crate::definitions::{Parent, TextRange};

/// The result of a lintspec operation
pub type Result<T> = std::result::Result<T, Error>;

/// A lintspec error
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Solidity version is not supported
    #[error("the provided Solidity version is not supported: `{0}`")]
    SolidityUnsupportedVersion(String),

    #[error("there was an error while parsing solidity: {0}")]
    ParsingError(String),

    /// Error during parsing of a version specifier string
    #[error("error parsing a semver string: {0}")]
    SemverParsingError(#[from] semver::Error),

    /// Error during parsing of a `NatSpec` comment
    #[error("error parsing a natspec comment: {message}")]
    NatspecParsingError {
        parent: Option<Parent>,
        span: TextRange,
        message: String,
    },

    /// IO error
    #[error("IO error for {path:?}: {err}")]
    IOError { path: PathBuf, err: std::io::Error },

    /// An unspecified error happening during parsing
    #[error("unknown error while parsing Solidity")]
    UnknownError,
}
