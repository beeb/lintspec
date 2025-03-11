//! Parsing and validation of struct definitions.
use slang_solidity::cst::TextRange;

use crate::{
    lint::{check_dev, check_notice, check_params, ItemDiagnostics},
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
        let opts = &options.structs;
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: Self::item_type(),
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        out.diags
            .extend(check_notice(&self.natspec, opts.notice, self.span()));
        out.diags
            .extend(check_dev(&self.natspec, opts.dev, self.span()));
        out.diags.extend(check_params(
            &self.natspec,
            opts.param,
            &self.members,
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

    static OPTIONS: LazyLock<ValidationOptions> = LazyLock::new(|| {
        ValidationOptions::builder()
            .inheritdoc(false)
            .structs(WithParamsRules::required())
            .build()
    });

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
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param a is missing");
        assert_eq!(res.diags[1].message, "@param b is missing");
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
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param a is missing");
    }

    #[test]
    fn test_struct_enforce() {
        let opts = ValidationOptions::builder()
            .structs(WithParamsRules {
                notice: Req::Required,
                dev: Req::default(),
                param: Req::default(),
            })
            .build();
        let contents = "contract Test {
            struct Foobar {
                uint256 a;
            }
        }";
        let res = parse_file(contents).validate(&opts);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@notice is missing");

        let contents = "contract Test {
            /// @notice Some notice
            struct Foobar {
                uint256 a;
            }
        }";
        let res = parse_file(contents).validate(&opts);
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
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param a is missing");
        assert_eq!(res.diags[1].message, "@param b is missing");
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
