//! Parsing and validation of struct definitions.
use slang_solidity::cst::{Cursor, Query, QueryMatch, TextRange};

use crate::{
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
    natspec::NatSpec,
};

use super::{
    capture, check_params, extract_comment, extract_parent_name, Definition, Identifier, Parent,
    Validate, ValidationOptions,
};

/// A struct definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct StructDefinition {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub members: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for StructDefinition {
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
            "@struct [StructDefinition
            @struct_name name:[Identifier]
            @struct_members members:[StructMembers]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let structure = capture(&m, "struct")?;
        let name = capture(&m, "struct_name")?;
        let members = capture(&m, "struct_members")?;

        let span = structure.text_range();
        let name = name.node().unparse().trim().to_string();
        let members = extract_struct_members(&members)?;
        let natspec = extract_comment(&structure.clone(), &[])?;
        let parent = extract_parent_name(structure);

        Ok(StructDefinition {
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
            item_type: ItemType::Struct,
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            if !options.struct_params && !options.enforce.contains(&ItemType::Struct) {
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

fn extract_struct_members(cursor: &Cursor) -> Result<Vec<Identifier>> {
    let cursor = cursor.spawn();
    let mut out = Vec::new();
    let query = Query::parse(
        "[StructMember
        @member_name name:[Identifier]
    ]",
    )
    .expect("query should compile");
    for m in cursor.query(vec![query]) {
        let member_name = capture(&m, "member_name")?;
        out.push(Identifier {
            name: Some(member_name.node().unparse().trim().to_string()),
            span: member_name.text_range(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use semver::Version;
    use similar_asserts::assert_eq;
    use slang_solidity::{cst::NonterminalKind, parser::Parser};

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
        def.as_struct().unwrap()
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
