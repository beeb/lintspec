//! Utils for parsing Solidity source code.
use std::sync::LazyLock;

use regex::Regex;
pub use semver;
use semver::{Version, VersionReq};
use slang_solidity::{
    cst::{NonterminalKind, Query, TextIndex},
    parser::Parser,
};

use crate::error::{Error, Result};

/// A regex to identify version pragma statements so that the whole file does not need to be parsed.
static REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"pragma\s+solidity[^;]+;").expect("the version pragma regex should compile")
});

/// Search for `pragma solidity` statements in the source and return the highest matching Solidity version.
///
/// If no pragma directive is found, the version defaults to `0.8.0`. Only the first pragma directive is considered,
/// other ones in the file are ignored. Multiple version specifiers separated by a space are taken as meaning "and",
/// specifiers separated by `||` are taken as meaning "or". Spaces take precedence over double-pipes.
///
/// Example: `0.6.0 || >=0.7.0 <0.8.0` means "either 0.6.0 or 0.7.x".
///
/// Within the specifiers' constraints, the highest version that is supported by [`slang_solidity`] is returned. In
/// the above example, version `0.7.6` would be used.
///
/// # Errors
/// This function errors if the found version string cannot be parsed to a [`VersionReq`] or if the version is not
/// supported by [`slang_solidity`].
///
/// # Panics
/// This function panics if the [`Parser::SUPPORTED_VERSIONS`] list is empty.
///
/// # Examples
///
/// ```
/// # use lintspec::utils::{detect_solidity_version, semver::Version};
/// assert_eq!(
///     detect_solidity_version("pragma solidity >=0.8.4 <0.8.26;").unwrap(),
///     Version::new(0, 8, 25)
/// );
/// assert_eq!(
///     detect_solidity_version("pragma solidity ^0.4.0 || 0.6.x;").unwrap(),
///     Version::new(0, 6, 12)
/// );
/// assert_eq!(
///     detect_solidity_version("contract Foo {}").unwrap(),
///     Version::new(0, 8, 0)
/// );
/// // this version of Solidity does not exist
/// assert!(detect_solidity_version("pragma solidity 0.7.7;").is_err());
/// ```
pub fn detect_solidity_version(src: &str) -> Result<Version> {
    let Some(pragma) = REGEX.find(src) else {
        return Ok(Version::new(0, 8, 0));
    };

    let parser = Parser::create(get_latest_supported_version())
        .expect("the Parser should be initialized correctly with a supported solidity version");

    let parse_result = parser.parse(NonterminalKind::PragmaDirective, pragma.as_str());
    if !parse_result.is_valid() {
        let Some(error) = parse_result.errors().first() else {
            return Err(Error::UnknownError);
        };
        return Err(Error::ParsingError(error.to_string()));
    }

    let cursor = parse_result.create_tree_cursor();
    let query_set = Query::parse("@version_set [VersionExpressionSet]")
        .expect("version set query should compile");
    let query_expr = Query::parse("@version_expr [VersionExpression]")
        .expect("version expr query should compile");

    let mut version_reqs = Vec::new();
    for m in cursor.query(vec![query_set]) {
        let Some((_, mut it)) = m.capture("version_set") else {
            continue;
        };
        let Some(set) = it.next() else {
            continue;
        };
        version_reqs.push(String::new());
        let cursor = set.node().cursor_with_offset(TextIndex::default());
        for m in cursor.query(vec![query_expr.clone()]) {
            let Some((_, mut it)) = m.capture("version_expr") else {
                continue;
            };
            let Some(expr) = it.next() else {
                continue;
            };
            let text = expr.node().unparse();
            let text = text.trim();
            // check if we are dealing with a version range with hyphen format
            if text.contains('-') {
                let (start, end) = text
                    .split_once('-')
                    .expect("version range should have a minus character");
                let v = version_reqs
                    .last_mut()
                    .expect("version expression should be inside an expression set");
                v.push_str(&format!(",>={},<={}", start.trim(), end.trim()));
            } else {
                let v = version_reqs
                    .last_mut()
                    .expect("version expression should be inside an expression set");
                // for `semver`, the different specifiers should be combined with a comma if they must all match
                if let Some(true) = text.chars().next().map(|c| c.is_ascii_digit()) {
                    // for `semver`, no comparator is the same as the caret comparator, but for solidity is means `=`
                    v.push_str(&format!(",={text}"));
                } else {
                    v.push_str(&format!(",{text}"));
                }
            }
        }
    }
    let reqs = version_reqs
        .into_iter()
        .map(|r| VersionReq::parse(r.trim_start_matches(',')).map_err(Into::into))
        .collect::<Result<Vec<_>>>()?;
    reqs.iter()
        .filter_map(|r| {
            Parser::SUPPORTED_VERSIONS
                .iter()
                .rev()
                .find(|v| r.matches(v))
        })
        .max()
        .cloned()
        .ok_or_else(|| Error::SolidityUnsupportedVersion(pragma.as_str().to_string()))
}

/// Get the latest Solidity version supported by the [`slang_solidity`] parser
#[must_use]
pub fn get_latest_supported_version() -> Version {
    Parser::SUPPORTED_VERSIONS
        .last()
        .expect("the SUPPORTED_VERSIONS list should not be empty")
        .to_owned()
}
