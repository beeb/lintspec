//! Parsing and validation of enum definitions.
use crate::{
    lint::{CheckNoticeAndDev, CheckParams, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{Identifier, ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions};

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
    fn item_type(&self) -> ItemType {
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
            item_type: self.item_type(),
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
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
                .params(&self.members)
                .default_span(self.span())
                .build()
                .check(),
        );
        out
    }
}

#[cfg(all(test, feature = "solar"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::LazyLock;

    use similar_asserts::assert_eq;

    use crate::{
        config::{Req, WithParamsRules},
        definitions::Definition,
        parser::{Parse as _, solar::SolarParser},
    };

    use super::*;

    static OPTIONS: LazyLock<ValidationOptions> = LazyLock::new(|| {
        ValidationOptions::builder()
            .enums(WithParamsRules::required())
            .inheritdoc(false)
            .build()
    });

    fn parse_file(contents: &str) -> EnumDefinition {
        let mut parser = SolarParser::default();
        let doc = parser
            .parse_document(contents.as_bytes(), None::<std::path::PathBuf>, false)
            .unwrap();
        doc.definitions
            .into_iter()
            .find_map(Definition::to_enumeration)
            .unwrap()
    }

    #[test]
    fn test_enum() {
        let contents = "contract Test {
            /// @notice The enum
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
        assert_eq!(res.diags.len(), 3);
        assert_eq!(res.diags[0].message, "@notice is missing");
        assert_eq!(res.diags[1].message, "@param First is missing");
        assert_eq!(res.diags[2].message, "@param Second is missing");
    }

    #[test]
    fn test_enum_params() {
        let contents = "contract Test {
            /// @notice The notice
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
             * @notice The enum
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
            /// @notice The enum
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
        // inheritdoc should be ignored as it doesn't apply to enums
        let contents = "contract Test {
            /// @inheritdoc Something
            enum Foobar {
                First
            }
        }";
        let res =
            parse_file(contents).validate(&ValidationOptions::builder().inheritdoc(true).build());
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@notice is missing");
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
            /// @notice An enum
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
        assert_eq!(res.diags.len(), 3);
        assert_eq!(res.diags[0].message, "@notice is missing");
        assert_eq!(res.diags[1].message, "@param First is missing");
        assert_eq!(res.diags[2].message, "@param Second is missing");
    }

    #[test]
    fn test_enum_no_contract_one_missing() {
        let contents = "
            /// @notice The enum
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
