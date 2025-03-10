//! Parsing and validation of constructors.
use slang_solidity::cst::TextRange;

use crate::{
    lint::{check_params, Diagnostic, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{Identifier, ItemType, Parent, SourceItem, Validate, ValidationOptions};

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
    fn item_type() -> ItemType {
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
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: Self::item_type(),
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        let Some(natspec) = &self.natspec else {
            if (!options.constructor || self.params.is_empty())
                && !options.enforce.contains(&Self::item_type())
            {
                return out;
            }
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "missing NatSpec".to_string(),
            });
            return out;
        };
        if !options.constructor || self.params.is_empty() {
            return out;
        }
        // check params
        out.diags.append(&mut check_params(natspec, &self.params));
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
        constructor: true,
        struct_params: false,
        enum_params: false,
        enforce: vec![],
    };

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
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
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
    fn test_constructor_inheritdoc() {
        let contents = "contract Test {
            /// @inheritdoc ITest
            constructor(uint256 param1) { }
        }";
        let res =
            parse_file(contents).validate(&ValidationOptions::builder().constructor(true).build());
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param param1 is missing");
    }

    #[test]
    fn test_constructor_enforce() {
        let contents = "contract Test {
            constructor() { }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .enforce(vec![ItemType::Constructor])
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");

        let contents = "contract Test {
            /// @notice Some notice
            constructor() { }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .enforce(vec![ItemType::Constructor])
                .build(),
        );
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }
}
