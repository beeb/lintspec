//! Parsing and validation of function definitions.
use slang_solidity::cst::TextRange;

use crate::{
    lint::{check_params, check_returns, Diagnostic, ItemDiagnostics},
    natspec::{NatSpec, NatSpecKind},
};

use super::{
    Attributes, Identifier, ItemType, Parent, SourceItem, Validate, ValidationOptions, Visibility,
};

/// A function definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct FunctionDefinition {
    /// The parent for the function definition (should always be `Some`)
    pub parent: Option<Parent>,

    /// The name of the function
    pub name: String,

    /// The span of the function definition, exluding the body
    pub span: TextRange,

    /// The name and span of the function's parameters
    pub params: Vec<Identifier>,

    /// The name and span of the function's returns
    pub returns: Vec<Identifier>,

    /// The [`NatSpec`] associated with the function definition, if any
    pub natspec: Option<NatSpec>,

    /// The attributes of the function (visibility and override)
    pub attributes: Attributes,
}

impl FunctionDefinition {
    /// Check whether this function requires inheritdoc when we enforce it
    ///
    /// External and public functions, as well as overridden internal functions must have inheritdoc.
    fn requires_inheritdoc(&self) -> bool {
        let parent_is_contract = matches!(self.parent, Some(Parent::Contract(_)));
        let internal_override =
            self.attributes.visibility == Visibility::Internal && self.attributes.r#override;
        let public_external = matches!(
            self.attributes.visibility,
            Visibility::External | Visibility::Public
        );
        parent_is_contract && (internal_override || public_external)
    }
}

impl SourceItem for FunctionDefinition {
    fn item_type() -> ItemType {
        ItemType::Function
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

impl Validate for FunctionDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: Self::item_type(),
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        // fallback and receive do not require NatSpec
        if self.name == "receive" || self.name == "fallback" {
            return out;
        }
        // raise error if no NatSpec is available (unless there are no params, no returns, and we don't enforce
        // inheritdoc or natspec at all)
        let Some(natspec) = &self.natspec else {
            if self.returns.is_empty()
                && self.params.is_empty()
                && !options.enforce.contains(&Self::item_type())
                && (!options.inheritdoc || !self.requires_inheritdoc())
            {
                return out;
            } else if options.inheritdoc && self.requires_inheritdoc() {
                out.diags.push(Diagnostic {
                    span: self.span(),
                    message: "@inheritdoc is missing".to_string(),
                });
                return out;
            }
            // we require natspec
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "missing NatSpec".to_string(),
            });
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
            return out;
        }
        // check params and returns
        out.diags.append(&mut check_params(natspec, &self.params));
        out.diags
            .append(&mut check_returns(natspec, &self.returns, self.span()));
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

    fn parse_file(contents: &str) -> FunctionDefinition {
        let parser = Parser::create(Version::new(0, 8, 0)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor
            .query(vec![FunctionDefinition::query()])
            .next()
            .unwrap();
        let def = FunctionDefinition::extract(m).unwrap();
        def.to_function().unwrap()
    }

    #[test]
    fn test_function() {
        let contents = "contract Test {
            /// @param param1 Test
            /// @param param2 Test2
            /// @return First output
            /// @return out Second output
            function foo(uint256 param1, bytes calldata param2) internal returns (uint256, uint256 out) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_function_no_natspec() {
        let contents = "contract Test {
            function foo(uint256 param1, bytes calldata param2) internal returns (uint256, uint256 out) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
    }

    #[test]
    fn test_function_only_notice() {
        let contents = "contract Test {
            /// @notice The function
            function foo(uint256 param1, bytes calldata param2) internal returns (uint256, uint256 out) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 4);
        assert_eq!(res.diags[0].message, "@param param1 is missing");
        assert_eq!(res.diags[1].message, "@param param2 is missing");
        assert_eq!(
            res.diags[2].message,
            "@return missing for unnamed return #1"
        );
        assert_eq!(res.diags[3].message, "@return out is missing");
    }

    #[test]
    fn test_function_one_missing() {
        let contents = "contract Test {
            /// @param param1 The first
            function foo(uint256 param1, bytes calldata param2) internal { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param param2 is missing");
    }

    #[test]
    fn test_function_multiline() {
        let contents = "contract Test {
            /**
             * @param param1 Test
             * @param param2 Test2
             */
            function foo(uint256 param1, bytes calldata param2) internal { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_function_duplicate() {
        let contents = "contract Test {
            /// @param param1 The first
            /// @param param1 The first again
            function foo(uint256 param1) internal { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(
            res.diags[0].message,
            "@param param1 is present more than once"
        );
    }

    #[test]
    fn test_function_duplicate_return() {
        let contents = "contract Test {
            /// @return out The output
            /// @return out The output again
            function foo() internal returns (uint256 out) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(
            res.diags[0].message,
            "@return out is present more than once"
        );
    }

    #[test]
    fn test_function_duplicate_unnamed_return() {
        let contents = "contract Test {
            /// @return The output
            /// @return The output again
            function foo() internal returns (uint256) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "too many unnamed returns");
    }

    #[test]
    fn test_function_no_params() {
        let contents = "contract Test {
            function foo() internal { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_requires_inheritdoc() {
        let contents = "contract Test is ITest {
            function a() internal returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(!res.requires_inheritdoc());

        let contents = "contract Test is ITest {
            function b() private returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(!res.requires_inheritdoc());

        let contents = "contract Test is ITest {
            function c() external returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc());

        let contents = "contract Test is ITest {
            function d() public returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc());

        let contents = "contract Test is ITest {
            function e() internal override (ITest) returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc());
    }

    #[test]
    fn test_function_inheritdoc() {
        let contents = "contract Test is ITest {
            /// @inheritdoc ITest
            function foo() external { }
        }";
        let res = parse_file(contents).validate(&ValidationOptions::default());
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_function_inheritdoc_missing() {
        let contents = "contract Test is ITest {
            /// @notice Test
            function foo() external { }
        }";
        let res = parse_file(contents).validate(&ValidationOptions::default());
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@inheritdoc is missing");
    }

    #[test]
    fn test_function_enforce() {
        let contents = "contract Test {
            function foo() internal { }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .enforce(vec![ItemType::Function])
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");

        let contents = "contract Test {
            /// @dev Some dev
            function foo() internal { }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .enforce(vec![ItemType::Function])
                .build(),
        );
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_function_internal() {
        let contents = "contract Test {
            // @notice
            function _viewInternal() internal view returns (uint256) {
                return 1;
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
    }
}
