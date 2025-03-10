//! Parsing and validation of struct definitions.
use slang_solidity::cst::TextRange;

use crate::{
    lint::{check_params, Diagnostic, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{Identifier, ItemType, Parent, SourceItem, Validate, ValidationOptions};

/// A struct definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct StructDefinition {
    /// The parent for the struct definition, if any
    pub parent: Option<Parent>,

    /// The name of the struct
    pub name: String,

    /// The span of the struct definition
    pub span: TextRange,

    /// The name and span of the struct members
    pub members: Vec<Identifier>,

    /// The [`NatSpec`] associated with the struct definition, if any
    pub natspec: Option<NatSpec>,
}

impl SourceItem for StructDefinition {
    fn item_type() -> ItemType {
        ItemType::Struct
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

impl Validate for StructDefinition {
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
            if !options.struct_params && !options.enforce.contains(&Self::item_type()) {
                return out;
            }
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "missing NatSpec".to_string(),
            });
            return out;
        };
        if !options.struct_params {
            return out;
        }
        out.diags = check_params(natspec, &self.members);
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
        struct_params: true,
        enum_params: false,
        enforce: vec![],
    };

    fn parse_file(contents: &str) -> StructDefinition {
        let parser = Parser::create(Version::new(0, 8, 0)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor
            .query(vec![StructDefinition::query()])
            .next()
            .unwrap();
        let def = StructDefinition::extract(m).unwrap();
        def.to_struct().unwrap()
    }

    #[test]
    fn test_struct() {
        let contents = "contract Test {
            struct Foobar {
                uint256 a;
                bool b;
            }
        }";
        let res =
            parse_file(contents).validate(&ValidationOptions::builder().inheritdoc(false).build());
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_struct_missing() {
        let contents = "contract Test {
            struct Foobar {
                uint256 a;
                bool b;
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
    }

    #[test]
    fn test_struct_params() {
        let contents = "contract Test {
            /// @param a The first
            /// @param b The second
            struct Foobar {
                uint256 a;
                bool b;
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_struct_only_notice() {
        let contents = "contract Test {
            /// @notice A struct
            struct Foobar {
                uint256 a;
                bool b;
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param a is missing");
        assert_eq!(res.diags[1].message, "@param b is missing");
    }

    #[test]
    fn test_struct_one_missing() {
        let contents = "contract Test {
            /// @notice A struct
            /// @param a The first
            struct Foobar {
                uint256 a;
                bool b;
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param b is missing");
    }

    #[test]
    fn test_struct_multiline() {
        let contents = "contract Test {
            /**
             * @param a The first
             * @param b The second
             */
            struct Foobar {
                uint256 a;
                bool b;
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_struct_duplicate() {
        let contents = "contract Test {
            /// @param a The first
            /// @param a The first twice
            struct Foobar {
                uint256 a;
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param a is present more than once");
    }

    #[test]
    fn test_struct_inheritdoc() {
        let contents = "contract Test {
            /// @inheritdoc
            struct Foobar {
                uint256 a;
            }
        }";
        let res = parse_file(contents)
            .validate(&ValidationOptions::builder().struct_params(true).build());
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param a is missing");
    }

    #[test]
    fn test_struct_enforce() {
        let contents = "contract Test {
            struct Foobar {
                uint256 a;
            }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .enforce(vec![ItemType::Struct])
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");

        let contents = "contract Test {
            /// @notice Some notice
            struct Foobar {
                uint256 a;
            }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .enforce(vec![ItemType::Struct])
                .build(),
        );
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_struct_no_contract() {
        let contents = "
            /// @param a The first
            /// @param b The second
            struct Foobar {
                uint256 a;
                bool b;
            }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_struct_no_contract_missing() {
        let contents = "struct Foobar {
                uint256 a;
                bool b;
            }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
    }

    #[test]
    fn test_struct_no_contract_one_missing() {
        let contents = "
            /// @param a The first
            struct Foobar {
                uint256 a;
                bool b;
            }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param b is missing");
    }
}
