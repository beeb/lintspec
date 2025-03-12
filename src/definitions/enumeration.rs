//! Parsing and validation of enum definitions.
use slang_solidity::cst::TextRange;

use crate::{
    lint::{check_notice_and_dev, check_params, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{Identifier, ItemType, Parent, SourceItem, Validate, ValidationOptions};

/// An enum definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct EnumDefinition {
    /// The parent for the enum definition, if any
    pub parent: Option<Parent>,

    /// The name of the enum
    pub name: String,

    /// The span of the enum definition
    pub span: TextRange,

    /// The name and span of the enum variants
    pub members: Vec<Identifier>,

    /// The [`NatSpec`] associated with the enum definition, if any
    pub natspec: Option<NatSpec>,
}

impl SourceItem for EnumDefinition {
    fn item_type() -> ItemType {
        ItemType::Enum
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

impl Validate for EnumDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let opts = &options.enums;
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: Self::item_type(),
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
            .enums(WithParamsRules::required())
            .build()
    });

    fn parse_file(contents: &str) -> EnumDefinition {
        let parser = Parser::create(Version::new(0, 8, 0)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor.query(vec![EnumDefinition::query()]).next().unwrap();
        let def = EnumDefinition::extract(m).unwrap();
        def.to_enum().unwrap()
    }

    #[test]
    fn test_enum() {
        let contents = "contract Test {
            enum Foobar {
                First,
                Second
            }
        }";
        let res =
            parse_file(contents).validate(&ValidationOptions::builder().inheritdoc(false).build());
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_enum_missing() {
        let contents = "contract Test {
            enum Foobar {
                First,
                Second
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param First is missing");
        assert_eq!(res.diags[1].message, "@param Second is missing");
    }

    #[test]
    fn test_enum_params() {
        let contents = "contract Test {
            /// @param First The first
            /// @param Second The second
            enum Foobar {
                First,
                Second
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_enum_only_notice() {
        let contents = "contract Test {
            /// @notice An enum
            enum Foobar {
                First,
                Second
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param First is missing");
        assert_eq!(res.diags[1].message, "@param Second is missing");
    }

    #[test]
    fn test_enum_one_missing() {
        let contents = "contract Test {
            /// @notice An enum
            /// @param First The first
            enum Foobar {
                First,
                Second
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param Second is missing");
    }

    #[test]
    fn test_enum_multiline() {
        let contents = "contract Test {
            /**
             * @param First The first
             * @param Second The second
             */
            enum Foobar {
                First,
                Second
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_enum_duplicate() {
        let contents = "contract Test {
            /// @param First The first
            /// @param First The first twice
            enum Foobar {
                First
            }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(
            res.diags[0].message,
            "@param First is present more than once"
        );
    }

    #[test]
    fn test_enum_inheritdoc() {
        let contents = "contract Test {
            /// @inheritdoc
            enum Foobar {
                First
            }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .enums(WithParamsRules::required())
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param First is missing");
    }

    #[test]
    fn test_enum_enforce() {
        let opts = ValidationOptions::builder()
            .enums(WithParamsRules {
                notice: Req::Required,
                dev: Req::default(),
                param: Req::default(),
            })
            .build();
        let contents = "contract Test {
            enum Foobar {
                First
            }
        }";
        let res = parse_file(contents).validate(&opts);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@notice is missing");

        let contents = "contract Test {
            /// @notice Some notice
            enum Foobar {
                First
            }
        }";
        let res = parse_file(contents).validate(&opts);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_enum_no_contract() {
        let contents = "
            /// @param First The first
            /// @param Second The second
            enum Foobar {
                First,
                Second
            }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_enum_no_contract_missing() {
        let contents = "enum Foobar {
                First,
                Second
            }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param First is missing");
        assert_eq!(res.diags[1].message, "@param Second is missing");
    }

    #[test]
    fn test_enum_no_contract_one_missing() {
        let contents = "
            /// @param First The first
            enum Foobar {
                First,
                Second
            }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param Second is missing");
    }
}
