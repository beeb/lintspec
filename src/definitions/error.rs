//! Parsing and validation of error definitions.
use slang_solidity::cst::TextRange;

use crate::{
    lint::{check_params, Diagnostic, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{Identifier, ItemType, Parent, SourceItem, Validate, ValidationOptions};

/// An error definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct ErrorDefinition {
    /// The parent for the error definition, if any
    pub parent: Option<Parent>,

    /// The name of the error
    pub name: String,

    /// The span of the error definition
    pub span: TextRange,

    /// The name and span of the error's parameters
    pub params: Vec<Identifier>,

    /// The [`NatSpec`] associated with the error definition, if any
    pub natspec: Option<NatSpec>,
}

impl SourceItem for ErrorDefinition {
    fn item_type() -> ItemType {
        ItemType::Error
    }

    fn parent(&self) -> Option<Parent> {
        self.parent.clone()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn span(&self) -> TextRange {
        self.span.clone()
    }
}

impl Validate for ErrorDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: Self::item_type(),
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            if self.params.is_empty() && !options.enforce.contains(&Self::item_type()) {
                return out;
            }
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "missing NatSpec".to_string(),
            });
            return out;
        };
        out.diags = check_params(natspec, &self.params);
        out
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;
    use similar_asserts::assert_eq;
    use slang_solidity::{cst::NonterminalKind, parser::Parser};

    use crate::parser::slang::Extract as _;

    use super::*;

    static OPTIONS: ValidationOptions = ValidationOptions {
        inheritdoc: false,
        constructor: false,
        struct_params: false,
        enum_params: false,
        enforce: vec![],
    };

    fn parse_file(contents: &str) -> ErrorDefinition {
        let parser = Parser::create(Version::new(0, 8, 26)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor.query(vec![ErrorDefinition::query()]).next().unwrap();
        let def = ErrorDefinition::extract(m).unwrap();
        def.to_error().unwrap()
    }

    #[test]
    fn test_error() {
        let contents = "contract Test {
            /// @param a The first
            /// @param b The second
            error Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_error_no_natspec() {
        let contents = "contract Test {
            error Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
    }

    #[test]
    fn test_error_only_notice() {
        let contents = "contract Test {
            /// @notice
            error Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param a is missing");
        assert_eq!(res.diags[1].message, "@param b is missing");
    }

    #[test]
    fn test_error_one_missing() {
        let contents = "contract Test {
            /// @param a The first
            error Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param b is missing");
    }

    #[test]
    fn test_error_multiline() {
        let contents = "contract Test {
            /**
             * @param a The first
             * @param b The second
             */
            error Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_error_duplicate() {
        let contents = "contract Test {
            /// @param a The first
            /// @param a The first again
            error Foobar(uint256 a);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param a is present more than once");
    }

    #[test]
    fn test_error_no_params() {
        let contents = "contract Test {
            error Foobar();
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_error_inheritdoc() {
        let contents = "contract Test {
            /// @inheritdoc ITest
            error Foobar(uint256 a);
        }";
        let res = parse_file(contents).validate(&ValidationOptions::default());
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param a is missing");
    }

    #[test]
    fn test_error_enforce() {
        let contents = "contract Test {
            error Foobar();
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(false)
                .enforce(vec![ItemType::Error])
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");

        let contents = "contract Test {
            /// @notice Some notice
            error Foobar();
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(false)
                .enforce(vec![ItemType::Error])
                .build(),
        );
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_error_no_contract() {
        let contents = "
            /// @param a The first
            /// @param b The second
            error Foobar(uint256 a, uint256 b);
            ";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_error_no_contract_missing() {
        let contents = "error Foobar(uint256 a, uint256 b);";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
    }
}
