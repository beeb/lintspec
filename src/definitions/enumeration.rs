use slang_solidity::cst::{Cursor, Query, QueryMatch, TerminalKind, TextRange};

use crate::{
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
    natspec::NatSpec,
};

use super::{
    capture, check_params, extract_comment, extract_parent_name, Definition, Identifier, Parent,
    Validate, ValidationOptions,
};

/// An enum definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct EnumDefinition {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub members: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for EnumDefinition {
    fn parent(&self) -> Option<Parent> {
        self.parent.clone()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn span(&self) -> TextRange {
        self.span.clone()
    }

    fn query() -> Query {
        Query::parse(
            "@enum [EnumDefinition
            @enum_name name:[Identifier]
            @enum_members members:[EnumMembers]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let enumeration = capture(&m, "enum")?;
        let name = capture(&m, "enum_name")?;
        let members = capture(&m, "enum_members")?;

        let span = enumeration.text_range();
        let name = name.node().unparse().trim().to_string();
        let members = extract_enum_members(members);
        let natspec = extract_comment(enumeration.clone(), &[])?;
        let parent = extract_parent_name(enumeration);

        Ok(EnumDefinition {
            parent,
            name,
            span,
            members,
            natspec,
        }
        .into())
    }

    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: ItemType::Enum,
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        if !options.enum_params {
            return out;
        }
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "missing NatSpec".to_string(),
            });
            return out;
        };
        out.diags = check_params(natspec, &self.members);
        out
    }
}

fn extract_enum_members(cursor: Cursor) -> Vec<Identifier> {
    let mut cursor = cursor.spawn().with_edges();
    let mut out = Vec::new();
    while cursor.go_to_next_terminal_with_kind(TerminalKind::Identifier) {
        out.push(Identifier {
            name: Some(cursor.node().unparse().trim().to_string()),
            span: cursor.text_range(),
        })
    }
    out
}

#[cfg(test)]
mod tests {
    use semver::Version;
    use slang_solidity::{cst::NonterminalKind, parser::Parser};

    use super::*;

    static OPTIONS: ValidationOptions = ValidationOptions {
        inheritdoc: false,
        constructor: false,
        struct_params: false,
        enum_params: true,
    };

    fn parse_file(contents: &str) -> EnumDefinition {
        let parser = Parser::create(Version::new(0, 8, 0)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor.query(vec![EnumDefinition::query()]).next().unwrap();
        let def = EnumDefinition::extract(m).unwrap();
        def.as_enum().unwrap()
    }

    #[test]
    fn test_enum() {
        let contents = "contract Test {
            enum Foobar {
                First,
                Second
            }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(false)
                .constructor(false)
                .enum_params(false)
                .struct_params(false)
                .build(),
        );
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
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
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
    fn test_enum_inheritdoc() {
        let contents = "contract Test {
            /// @inheritdoc
            enum Foobar {
                First
            }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(true) // has no effect for enums
                .constructor(false)
                .enum_params(true)
                .struct_params(false)
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param First is missing");
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
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
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
