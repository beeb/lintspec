//! Parsing and validation of function definitions.
use crate::{
    interner::{INTERNER, Symbol},
    lint::{CheckNoticeAndDev, CheckParams, CheckReturns, Diagnostic, ItemDiagnostics},
    natspec::{NatSpec, NatSpecKind},
};

use super::{
    Attributes, Identifier, ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions,
    Visibility,
};

/// A function definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct FunctionDefinition {
    /// The parent for the function definition (should always be `Some`)
    pub parent: Option<Parent>,

    /// The name of the function
    pub name: Symbol,

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
    /// Check whether this function requires inheritdoc
    ///
    /// Public/external functions as well as internal override functions require @inheritdoc if enforced.
    fn requires_inheritdoc(&self, options: &ValidationOptions) -> bool {
        let parent_is_contract = matches!(self.parent, Some(Parent::Contract(_)));
        let internal_override =
            self.attributes.visibility == Visibility::Internal && self.attributes.r#override;
        let public_external = matches!(
            self.attributes.visibility,
            Visibility::External | Visibility::Public
        );
        ((options.inheritdoc && public_external)
            || (options.inheritdoc_override && internal_override))
            && parent_is_contract
    }
}

impl SourceItem for FunctionDefinition {
    fn item_type(&self) -> ItemType {
        match self.attributes.visibility {
            Visibility::External => ItemType::ExternalFunction,
            Visibility::Internal => ItemType::InternalFunction,
            Visibility::Private => ItemType::PrivateFunction,
            Visibility::Public => ItemType::PublicFunction,
        }
    }

    fn parent(&self) -> Option<Parent> {
        self.parent.clone()
    }

    fn name(&self) -> Symbol {
        self.name
    }

    fn span(&self) -> TextRange {
        self.span.clone()
    }
}

impl Validate for FunctionDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let name = self.name.resolve_with(&INTERNER);
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: self.item_type(),
            name,
            span: self.span(),
            diags: vec![],
        };
        // fallback and receive do not require NatSpec
        if name == "receive" || name == "fallback" {
            return out;
        }
        let opts = match self.attributes.visibility {
            Visibility::External => options.functions.external,
            Visibility::Internal => options.functions.internal,
            Visibility::Private => options.functions.private,
            Visibility::Public => options.functions.public,
        };
        if let Some(natspec) = &self.natspec
            && natspec
                .items
                .iter()
                .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
            // if there is `inheritdoc`, no further validation is required
            return out;
        } else if self.requires_inheritdoc(options) {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "@inheritdoc is missing".to_string(),
            });
            return out;
        }
        out.diags.extend(
            CheckNoticeAndDev::builder()
                .natspec(&self.natspec)
                .notice_rule(opts.notice)
                .dev_rule(opts.dev)
                .notice_or_dev(options.notice_or_dev)
                .span(&self.span)
                .build()
                .check(),
        );
        out.diags.extend(
            CheckParams::builder()
                .natspec(&self.natspec)
                .rule(opts.param)
                .params(&self.params)
                .default_span(self.span())
                .build()
                .check(),
        );
        out.diags.extend(
            CheckReturns::builder()
                .natspec(&self.natspec)
                .rule(opts.returns)
                .returns(&self.returns)
                .default_span(self.span())
                .is_var(false)
                .build()
                .check(),
        );
        out
    }
}

#[cfg(test)]
#[cfg(feature = "solar")]
mod tests {
    use std::sync::LazyLock;

    use similar_asserts::assert_eq;

    use crate::{
        definitions::Definition,
        parser::{Parse as _, solar::SolarParser},
    };

    use super::*;

    static OPTIONS: LazyLock<ValidationOptions> =
        LazyLock::new(|| ValidationOptions::builder().inheritdoc(false).build());

    fn parse_file(contents: &str) -> FunctionDefinition {
        let mut parser = SolarParser::default();
        let doc = parser
            .parse_document(contents.as_bytes(), None::<std::path::PathBuf>, false)
            .unwrap();
        doc.definitions
            .into_iter()
            .find_map(Definition::to_function)
            .unwrap()
    }

    #[test]
    fn test_function() {
        let contents = "contract Test {
            /// @notice A function
            /// @param param1 Test
            /// @param param2 Test2
            /// @return First output
            /// @return out Second output
            function foo(uint256 param1, bytes calldata param2) public returns (uint256, uint256 out) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_function_no_natspec() {
        let contents = "contract Test {
            function foo(uint256 param1, bytes calldata param2) public returns (uint256, uint256 out) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 5);
        assert_eq!(res.diags[0].message, "@notice is missing");
        assert_eq!(res.diags[1].message, "@param param1 is missing");
        assert_eq!(res.diags[2].message, "@param param2 is missing");
        assert_eq!(
            res.diags[3].message,
            "@return missing for unnamed return #1"
        );
        assert_eq!(res.diags[4].message, "@return out is missing");
    }

    #[test]
    fn test_function_only_notice() {
        let contents = "contract Test {
            /// @notice The function
            function foo(uint256 param1, bytes calldata param2) public returns (uint256, uint256 out) { }
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
            /// @notice A function
            /// @param param1 The first
            function foo(uint256 param1, bytes calldata param2) public { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param param2 is missing");
    }

    #[test]
    fn test_function_multiline() {
        let contents = "contract Test {
            /**
             * @notice A function
             * @param param1 Test
             * @param param2 Test2
             */
            function foo(uint256 param1, bytes calldata param2) public { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_function_duplicate() {
        let contents = "contract Test {
            /// @notice A function
            /// @param param1 The first
            /// @param param1 The first again
            function foo(uint256 param1) public { }
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
            /// @notice A function
            /// @return out The output
            /// @return out The output again
            function foo() public returns (uint256 out) { }
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
            /// @notice A function
            /// @return The output
            /// @return The output again
            function foo() public returns (uint256) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "too many unnamed returns");
    }

    #[test]
    fn test_function_no_params() {
        let contents = "contract Test {
            function foo() public { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@notice is missing");
    }

    #[test]
    fn test_requires_inheritdoc() {
        let options = ValidationOptions::builder()
            .inheritdoc(true)
            .inheritdoc_override(true)
            .build();
        let contents = "contract Test is ITest {
            function a() internal returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(!res.requires_inheritdoc(&options));

        let contents = "contract Test is ITest {
            function b() private returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(!res.requires_inheritdoc(&options));

        let contents = "contract Test is ITest {
            function c() external returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc(&options));
        assert!(!res.requires_inheritdoc(&ValidationOptions::builder().inheritdoc(false).build()));

        let contents = "contract Test is ITest {
            function d() public returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc(&options));
        assert!(!res.requires_inheritdoc(&ValidationOptions::builder().inheritdoc(false).build()));

        let contents = "contract Test is ITest {
            function e() internal override (ITest) returns (uint256) { }
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc(&options));
        assert!(!res.requires_inheritdoc(&ValidationOptions::default()));
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
}
