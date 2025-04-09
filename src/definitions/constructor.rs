//! Parsing and validation of constructors.
use crate::{
    lint::{check_notice_and_dev, check_params, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{Identifier, ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions};

/// A constructor definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
pub struct ConstructorDefinition {
    /// The parent contract (should always be a [`Parent::Contract`])
    pub parent: Option<Parent>,

    /// The span corresponding to the constructor definition, excluding the body
    pub span: TextRange,

    /// The name and span of the constructor's parameters
    pub params: Vec<Identifier>,

    /// The [`NatSpec`] associated with the constructor, if any
    pub natspec: Option<NatSpec>,
}

impl SourceItem for ConstructorDefinition {
    fn item_type(&self) -> ItemType {
        ItemType::Constructor
    }

    fn parent(&self) -> Option<Parent> {
        self.parent.clone()
    }

    fn name(&self) -> String {
        "constructor".to_string()
    }

    fn span(&self) -> TextRange {
        self.span.clone()
    }
}

impl Validate for ConstructorDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let opts = &options.constructors;
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: self.item_type(),
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        out.diags.extend(check_notice_and_dev(
            &self.natspec,
            opts.notice,
            opts.dev,
            options.notice_or_dev,
            self.span(),
        ));
        out.diags.extend(check_params(
            &self.natspec,
            opts.param,
            &self.params,
            self.span(),
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use semver::Version;
    use similar_asserts::assert_eq;
    use slang_solidity::{cst::NonterminalKind, parser::Parser};

    use crate::{
        config::{Req, WithParamsRules},
        parser::slang::Extract as _,
    };

    use super::*;

    static OPTIONS: LazyLock<ValidationOptions> =
        LazyLock::new(|| ValidationOptions::builder().inheritdoc(false).build());

    fn parse_file(contents: &str) -> ConstructorDefinition {
        let parser = Parser::create(Version::new(0, 8, 0)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor
            .query(vec![ConstructorDefinition::query()])
            .next()
            .unwrap();
        let def = ConstructorDefinition::extract(m).unwrap();
        def.to_constructor().unwrap()
    }

    #[test]
    fn test_constructor() {
        let contents = "contract Test {
            /// @param param1 Test
            /// @param param2 Test2
            constructor(uint256 param1, bytes calldata param2) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_constructor_no_natspec() {
        let contents = "contract Test {
            constructor(uint256 param1, bytes calldata param2) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param param1 is missing");
        assert_eq!(res.diags[1].message, "@param param2 is missing");
    }

    #[test]
    fn test_constructor_only_notice() {
        let contents = "contract Test {
            /// @notice The constructor
            constructor(uint256 param1, bytes calldata param2) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param param1 is missing");
        assert_eq!(res.diags[1].message, "@param param2 is missing");
    }

    #[test]
    fn test_constructor_one_missing() {
        let contents = "contract Test {
            /// @param param1 The first
            constructor(uint256 param1, bytes calldata param2) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param param2 is missing");
    }

    #[test]
    fn test_constructor_multiline() {
        let contents = "contract Test {
            /**
             * @param param1 Test
             * @param param2 Test2
             */
            constructor(uint256 param1, bytes calldata param2) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_constructor_duplicate() {
        let contents = "contract Test {
            /// @param param1 The first
            /// @param param1 The first again
            constructor(uint256 param1) { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(
            res.diags[0].message,
            "@param param1 is present more than once"
        );
    }

    #[test]
    fn test_constructor_no_params() {
        let contents = "contract Test {
            constructor() { }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_constructor_notice() {
        // inheritdoc should be ignored since it's a constructor
        let contents = "contract Test {
            /// @inheritdoc ITest
            constructor(uint256 param1) { }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(true)
                .constructors(WithParamsRules::required())
                .build(),
        );
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@notice is missing");
        assert_eq!(res.diags[1].message, "@param param1 is missing");
    }

    #[test]
    fn test_constructor_enforce() {
        let opts = ValidationOptions::builder()
            .constructors(WithParamsRules {
                notice: Req::Required,
                dev: Req::default(),
                param: Req::default(),
            })
            .build();
        let contents = "contract Test {
            constructor() { }
        }";
        let res = parse_file(contents).validate(&opts);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@notice is missing");

        let contents = "contract Test {
            /// @notice Some notice
            constructor() { }
        }";
        let res = parse_file(contents).validate(&opts);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }
}
