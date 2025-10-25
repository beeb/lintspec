//! Parsing and validation of struct definitions.
use crate::{
    lint::{CheckNoticeAndDev, CheckParams, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{Identifier, ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions};

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
    fn item_type(&self) -> ItemType {
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
                .span(self.span())
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
mod tests {
    use std::sync::LazyLock;

    use similar_asserts::assert_eq;

    use crate::{
        config::WithParamsRules,
        definitions::Definition,
        parser::{Parse as _, solar::SolarParser},
    };

    use super::*;

    static OPTIONS: LazyLock<ValidationOptions> = LazyLock::new(|| {
        ValidationOptions::builder()
            .inheritdoc(false)
            .structs(WithParamsRules::required())
            .build()
    });

    fn parse_file(contents: &str) -> StructDefinition {
        let mut parser = SolarParser::default();
        let doc = parser
            .parse_document(contents.as_bytes(), None::<std::path::PathBuf>, false)
            .unwrap();
        doc.definitions
            .into_iter()
            .find_map(Definition::to_struct)
            .unwrap()
    }

    #[test]
    fn test_struct() {
        let contents = "contract Test {
            /// @notice A struct
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
        assert_eq!(res.diags.len(), 3);
        assert_eq!(res.diags[0].message, "@notice is missing");
        assert_eq!(res.diags[1].message, "@param a is missing");
        assert_eq!(res.diags[2].message, "@param b is missing");
    }

    #[test]
    fn test_struct_params() {
        let contents = "contract Test {
            /// @notice A struct
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
             * @notice A struct
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
            /// @notice A struct
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
        // inheritdoc should be ignored as it doesn't apply to structs
        let contents = "contract Test {
            /// @inheritdoc ISomething
            struct Foobar {
                uint256 a;
            }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(true)
                .structs(WithParamsRules::required())
                .build(),
        );
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@notice is missing");
        assert_eq!(res.diags[1].message, "@param a is missing");
    }

    #[test]
    fn test_struct_no_contract() {
        let contents = "
            /// @notice A struct
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
        assert_eq!(res.diags.len(), 3);
        assert_eq!(res.diags[0].message, "@notice is missing");
        assert_eq!(res.diags[1].message, "@param a is missing");
        assert_eq!(res.diags[2].message, "@param b is missing");
    }

    #[test]
    fn test_struct_no_contract_one_missing() {
        let contents = "
            /// @notice A struct
            /// @param a The first
            struct Foobar {
                uint256 a;
                bool b;
            }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param b is missing");
    }

    #[test]
    fn test_struct_missing_space() {
        let contents = "
            /// @notice A struct
            /// @param fooThe param
            struct Test {
                uint256 foo;
            }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "extra @param fooThe");
        assert_eq!(res.diags[1].message, "@param foo is missing");
    }

    #[test]
    fn test_struct_extra_param() {
        let contents = "
            /// @notice A struct
            /// @param foo The param
            /// @param bar Some other param
            struct Test {
                uint256 foo;
            }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "extra @param bar");
    }
}
