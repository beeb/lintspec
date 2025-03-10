//! Parsing and validation of state variable declarations.
use slang_solidity::cst::TextRange;

use crate::{
    lint::{Diagnostic, ItemDiagnostics},
    natspec::{NatSpec, NatSpecKind},
};

use super::{Attributes, ItemType, Parent, SourceItem, Validate, ValidationOptions, Visibility};

/// A state variable declaration
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct VariableDeclaration {
    /// The parent for the state variable declaration (should always be `Some`)
    pub parent: Option<Parent>,

    /// The name of the state variable
    pub name: String,

    /// The span of the state variable declaration
    pub span: TextRange,

    /// The [`NatSpec`] associated with the state variable declaration, if any
    pub natspec: Option<NatSpec>,

    /// The attributes of the state variable (visibility)
    pub attributes: Attributes,
}

impl VariableDeclaration {
    /// Check whether this variable requires inheritdoc when we enforce it
    ///
    /// Public state variables must have inheritdoc.
    fn requires_inheritdoc(&self) -> bool {
        let parent_is_contract = matches!(self.parent, Some(Parent::Contract(_)));
        let public = self.attributes.visibility == Visibility::Public;
        parent_is_contract && public
    }
}

impl SourceItem for VariableDeclaration {
    fn item_type() -> ItemType {
        ItemType::Variable
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

impl Validate for VariableDeclaration {
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
            if options.inheritdoc && self.requires_inheritdoc() {
                out.diags.push(Diagnostic {
                    span: self.span(),
                    message: "@inheritdoc is missing".to_string(),
                });
            } else if options.enforce.contains(&Self::item_type()) {
                out.diags.push(Diagnostic {
                    span: self.span(),
                    message: "missing NatSpec".to_string(),
                });
            }
            return out;
        };
        // if there is `inheritdoc`, no further validation is required
        if natspec
            .items
            .iter()
            .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
            return out;
        } else if options.inheritdoc && self.requires_inheritdoc() {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "@inheritdoc is missing".to_string(),
            });
        }
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
        inheritdoc: true,
        constructor: false,
        struct_params: false,
        enum_params: false,
        enforce: vec![],
    };

    fn parse_file(contents: &str) -> VariableDeclaration {
        let parser = Parser::create(Version::new(0, 8, 0)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor
            .query(vec![VariableDeclaration::query()])
            .next()
            .unwrap();
        let def = VariableDeclaration::extract(m).unwrap();
        def.to_variable().unwrap()
    }

    #[test]
    fn test_variable() {
        let contents = "contract Test is ITest {
            /// @inheritdoc ITest
            uint256 public a;
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_variable_no_natspec_inheritdoc() {
        let contents = "contract Test {
            uint256 public a;
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@inheritdoc is missing");
    }

    #[test]
    fn test_variable_only_notice() {
        let contents = "contract Test {
            /// @notice The variable
            uint256 public a;
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@inheritdoc is missing");
    }

    #[test]
    fn test_variable_multiline() {
        let contents = "contract Test is ITest {
            /**
             * @inheritdoc ITest
             */
            uint256 public a;
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_requires_inheritdoc() {
        let contents = "contract Test is ITest {
            uint256 public a;
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc());

        let contents = "contract Test is ITest {
            uint256 internal a;
        }";
        let res = parse_file(contents);
        assert!(!res.requires_inheritdoc());
    }

    #[test]
    fn test_variable_no_inheritdoc() {
        let contents = "contract Test {
            uint256 internal a;
        }";
        let res =
            parse_file(contents).validate(&ValidationOptions::builder().inheritdoc(false).build());
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_variable_enforce() {
        let contents = "contract Test {
            uint256 internal a;
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(false)
                .enforce(vec![ItemType::Variable])
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");

        let contents = "contract Test {
            /// @dev Some dev
            uint256 internal a;
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(false)
                .enforce(vec![ItemType::Variable])
                .build(),
        );
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }
}
