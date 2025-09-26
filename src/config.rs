//! Tool configuration parsing and validation
use std::path::PathBuf;

use derive_more::IsVariant;
use figment::{
    Figment, Metadata, Profile, Provider,
    providers::{Env, Format as _, Toml},
    value::{Dict, Map},
};
use serde::{Deserialize, Serialize};

/// The requirement for a specific tag in the natspec comment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, IsVariant)]
#[serde(rename_all = "lowercase")]
pub enum Req {
    /// The tag is ignored, can be present or not
    #[default]
    Ignored,

    /// The tag is required, must be present
    Required,

    /// The tag is forbidden, must not be present
    Forbidden,
}

impl Req {
    /// Helper function to check if the tag is required or ignored (not forbidden)
    #[must_use]
    pub fn is_required_or_ignored(&self) -> bool {
        match self {
            Req::Required | Req::Ignored => true,
            Req::Forbidden => false,
        }
    }

    /// Helper function to check if the tag is forbidden or ignored (not required)
    #[must_use]
    pub fn is_forbidden_or_ignored(&self) -> bool {
        match self {
            Req::Forbidden | Req::Ignored => true,
            Req::Required => false,
        }
    }
}

/// Validation rules for a function natspec comment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct FunctionRules {
    /// Requirement for the `@notice` tag
    #[builder(default = Req::Required)]
    pub notice: Req,

    /// Requirement for the `@dev` tag
    #[builder(default)]
    pub dev: Req,

    /// Requirement for the `@param` tags
    #[builder(default = Req::Required)]
    pub param: Req,

    /// Requirement for the `@return` tags
    #[serde(rename = "return")]
    #[builder(default = Req::Required)]
    pub returns: Req,
}

impl Default for FunctionRules {
    fn default() -> Self {
        Self {
            notice: Req::Required,
            dev: Req::default(),
            param: Req::Required,
            returns: Req::Required,
        }
    }
}

/// Validation rules for each function visibility (private, internal, public, external)
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct FunctionConfig {
    /// Rules for private functions
    #[builder(default)]
    pub private: FunctionRules,

    /// Rules for internal functions
    #[builder(default)]
    pub internal: FunctionRules,

    /// Rules for public functions
    #[builder(default)]
    pub public: FunctionRules,

    /// Rules for external functions
    #[builder(default)]
    pub external: FunctionRules,
}

/// Validation rules for items which have return values but no params (public state variables)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct WithReturnsRules {
    /// Requirement for the `@notice` tag
    #[builder(default = Req::Required)]
    pub notice: Req,

    /// Requirement for the `@dev` tag
    #[builder(default)]
    pub dev: Req,

    /// Requirement for the `@param` tags
    #[serde(rename = "return")]
    #[builder(default = Req::Required)]
    pub returns: Req,
}

impl Default for WithReturnsRules {
    fn default() -> Self {
        Self {
            notice: Req::Required,
            dev: Req::default(),
            returns: Req::Required,
        }
    }
}

/// Validation rules for items which have no return values and no params (private and internal state variables)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct NoticeDevRules {
    #[builder(default = Req::Required)]
    pub notice: Req,
    #[builder(default)]
    pub dev: Req,
}

impl Default for NoticeDevRules {
    fn default() -> Self {
        Self {
            notice: Req::Required,
            dev: Req::default(),
        }
    }
}

/// Validation rules for each state variable visibility (private, internal, public)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct VariableConfig {
    #[builder(default)]
    pub private: NoticeDevRules,
    #[builder(default)]
    pub internal: NoticeDevRules,
    #[builder(default)]
    pub public: WithReturnsRules,
}

/// Validation rules for items which have params but no returns (constructor, enum, error, event, modifier, struct)
///
/// The default value does not enforce that `@param` is present, because it's not part of the official spec for enums
/// and structs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct WithParamsRules {
    #[builder(default = Req::Required)]
    pub notice: Req,
    #[builder(default)]
    pub dev: Req,
    #[builder(default)]
    pub param: Req,
}

impl Default for WithParamsRules {
    fn default() -> Self {
        Self {
            notice: Req::Required,
            dev: Req::default(),
            param: Req::default(),
        }
    }
}

impl WithParamsRules {
    /// Helper function to get a set of rules which enforces params (default for error, event, modifier)
    #[must_use]
    pub fn required() -> Self {
        Self {
            param: Req::Required,
            ..Default::default()
        }
    }

    /// Helper function to get a default set of rules for constructors (`@notice` is not enforced, but `@param` is)
    #[must_use]
    pub fn default_constructor() -> Self {
        Self {
            notice: Req::Ignored,
            param: Req::Required,
            ..Default::default()
        }
    }
}

/// General config for the tool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)]
pub struct BaseConfig {
    /// Paths to files and folders to analyze
    #[builder(default)]
    pub paths: Vec<PathBuf>,

    /// Paths to files and folders to exclude
    #[builder(default)]
    pub exclude: Vec<PathBuf>,

    /// Enforce that all public and external items have `@inheritdoc`
    #[builder(default = true)]
    pub inheritdoc: bool,

    /// Enforce that internal functions and modifiers which are `override` have `@inheritdoc`
    #[builder(default = false)]
    pub inheritdoc_override: bool,

    /// Do not distinguish between `@notice` and `@dev` when considering "required" validation rules
    #[builder(default)]
    pub notice_or_dev: bool,

    /// Skip the detection of the Solidity version and use the latest version supported by [`slang_solidity`]
    #[builder(default)]
    pub skip_version_detection: bool,
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            paths: Vec::default(),
            exclude: Vec::default(),
            inheritdoc: true,
            inheritdoc_override: false,
            notice_or_dev: false,
            skip_version_detection: false,
        }
    }
}

/// Output config for the tool
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct OutputConfig {
    /// Path to a file to write the output to (instead of stderr)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out: Option<PathBuf>,

    /// Output diagnostics in JSON format
    #[builder(default)]
    pub json: bool,

    /// Compact output (minified JSON or compact text representation)
    #[builder(default)]
    pub compact: bool,

    /// Sort the results by file path
    #[builder(default)]
    pub sort: bool,
}

/// The parsed and validated config for the tool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct Config {
    /// General config for the tool
    #[builder(default)]
    pub lintspec: BaseConfig,

    /// Output config for the tool
    #[builder(default)]
    pub output: OutputConfig,

    /// Validation rules for constructors
    #[serde(rename = "constructor")]
    #[builder(default = WithParamsRules::default_constructor())]
    pub constructors: WithParamsRules,

    /// Validation rules for enums
    #[serde(rename = "enum")]
    #[builder(default)]
    pub enums: WithParamsRules,

    /// Validation rules for errors
    #[serde(rename = "error")]
    #[builder(default = WithParamsRules::required())]
    pub errors: WithParamsRules,

    /// Validation rules for events
    #[serde(rename = "event")]
    #[builder(default = WithParamsRules::required())]
    pub events: WithParamsRules,

    /// Validation rules for functions
    #[serde(rename = "function")]
    #[builder(default)]
    pub functions: FunctionConfig,

    /// Validation rules for modifiers
    #[serde(rename = "modifier")]
    #[builder(default = WithParamsRules::required())]
    pub modifiers: WithParamsRules,

    /// Validation rules for structs
    #[serde(rename = "struct")]
    #[builder(default)]
    pub structs: WithParamsRules,

    /// Validation rules for state variables
    #[serde(rename = "variable")]
    #[builder(default)]
    pub variables: VariableConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lintspec: BaseConfig::default(),
            output: OutputConfig::default(),
            constructors: WithParamsRules::default_constructor(),
            enums: WithParamsRules::default(),
            errors: WithParamsRules::required(),
            events: WithParamsRules::required(),
            functions: FunctionConfig::default(),
            modifiers: WithParamsRules::required(),
            structs: WithParamsRules::default(),
            variables: VariableConfig::default(),
        }
    }
}

impl Config {
    pub fn from(provider: impl Provider) -> Result<Config, Box<figment::Error>> {
        Figment::from(provider).extract().map_err(Box::new)
    }

    /// Create a Figment which reads the config from the default file and environment variables
    #[must_use]
    pub fn figment(config_path: Option<PathBuf>) -> Figment {
        Figment::from(Config::default())
            .admerge(Toml::file(config_path.unwrap_or(".lintspec.toml".into())))
            .admerge(Env::prefixed("LS_").split("_").map(|k| {
                // special case for parameters with an underscore in the name
                match k.as_str() {
                    "LINTSPEC.NOTICE.OR.DEV" => "LINTSPEC.NOTICE_OR_DEV".into(),
                    "LINTSPEC.SKIP.VERSION.DETECTION" => "LINTSPEC.SKIP_VERSION_DETECTION".into(),
                    _ => k.into(),
                }
            }))
    }
}

/// Implement [`Provider`] for composability
impl Provider for Config {
    fn metadata(&self) -> figment::Metadata {
        Metadata::named("LintSpec Config")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        figment::providers::Serialized::defaults(Config::default()).data()
    }
}

#[cfg(test)]
mod tests {
    use similar_asserts::assert_eq;

    use super::*;

    #[test]
    fn test_default_builder() {
        assert_eq!(FunctionRules::default(), FunctionRules::builder().build());
        assert_eq!(FunctionConfig::default(), FunctionConfig::builder().build());
        assert_eq!(
            WithReturnsRules::default(),
            WithReturnsRules::builder().build()
        );
        assert_eq!(NoticeDevRules::default(), NoticeDevRules::builder().build());
        assert_eq!(VariableConfig::default(), VariableConfig::builder().build());
        assert_eq!(
            WithParamsRules::default(),
            WithParamsRules::builder().build()
        );
        assert_eq!(BaseConfig::default(), BaseConfig::builder().build());
        assert_eq!(OutputConfig::default(), OutputConfig::builder().build());
        assert_eq!(Config::default(), Config::builder().build());
    }
}
