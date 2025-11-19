//! Parsing and validation of state variable declarations.
use crate::{
    lint::{CheckNoticeAndDev, CheckReturns, Diagnostic, ItemDiagnostics},
    natspec::{NatSpec, NatSpecKind},
};

use super::{
    Attributes, Identifier, ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions,
    Visibility,
};

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
    fn item_type(&self) -> ItemType {
        match self.attributes.visibility {
            Visibility::External => unreachable!("variables cannot be external"),
            Visibility::Internal => ItemType::InternalVariable,
            Visibility::Private => ItemType::PrivateVariable,
            Visibility::Public => ItemType::PublicVariable,
        }
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
        let (notice, dev, returns) = match self.attributes.visibility {
            Visibility::External => unreachable!("variables cannot be external"),
            Visibility::Internal => (
                options.variables.internal.notice,
                options.variables.internal.dev,
                None,
            ),
            Visibility::Private => (
                options.variables.private.notice,
                options.variables.private.dev,
                None,
            ),
            Visibility::Public => (
                options.variables.public.notice,
                options.variables.public.dev,
                Some(options.variables.public.returns),
            ),
        };
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: self.item_type(),
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        if let Some(natspec) = &self.natspec
            && natspec
                .items
                .iter()
                .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
            // if there is `inheritdoc`, no further validation is required
            return out;
        } else if options.inheritdoc && self.requires_inheritdoc() {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "@inheritdoc is missing".to_string(),
            });
            return out;
        }
        out.diags.extend(
            CheckNoticeAndDev::builder()
                .natspec(&self.natspec)
                .notice_rule(notice)
                .dev_rule(dev)
                .notice_or_dev(options.notice_or_dev)
                .span(&self.span)
                .build()
                .check(),
        );
        if let Some(returns) = returns {
            out.diags.extend(
                CheckReturns::builder()
                    .natspec(&self.natspec)
                    .rule(returns)
                    .returns(&[Identifier {
                        name: None,
                        span: self.span(),
                    }])
                    .default_span(self.span())
                    .is_var(true)
                    .build()
                    .check(),
            );
        }
        out
    }
}

#[cfg(all(test, feature = "solar"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::LazyLock;

    use similar_asserts::assert_eq;

    use crate::{
        definitions::Definition,
        parser::{Parse as _, solar::SolarParser},
    };

    use super::*;

    static OPTIONS: LazyLock<ValidationOptions> = LazyLock::new(Default::default);

    fn parse_file(contents: &str) -> VariableDeclaration {
        let mut parser = SolarParser::default();
        let doc = parser
            .parse_document(contents.as_bytes(), None::<std::path::PathBuf>, false)
            .unwrap();
        doc.definitions
            .into_iter()
            .find_map(Definition::to_variable)
            .unwrap()
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
    fn test_variable_no_natspec() {
        let contents = "contract Test {
            uint256 public a;
        }";
        let res =
            parse_file(contents).validate(&ValidationOptions::builder().inheritdoc(false).build());
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@notice is missing");
        assert_eq!(res.diags[1].message, "@return is missing");
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
            /// @notice A variable
            uint256 internal a;
        }";
        let res =
            parse_file(contents).validate(&ValidationOptions::builder().inheritdoc(false).build());
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }
}
