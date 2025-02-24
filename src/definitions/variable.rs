use slang_solidity::cst::{Query, QueryMatch, TextRange};

use crate::{
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
    natspec::{NatSpec, NatSpecKind},
};

use super::{
    capture, extract_attributes, extract_comment, extract_parent_name, Attributes, Definition,
    Parent, Validate, ValidationOptions, Visibility,
};

/// A state variable declaration
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct VariableDeclaration {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub natspec: Option<NatSpec>,
    pub attributes: Attributes,
}

impl VariableDeclaration {
    /// Check whether this variable requires inheritdoc when we enforce it
    ///
    /// Public state variables must have inheritdoc.
    fn requires_inheritdoc(&self) -> bool {
        let parent_is_contract = matches!(self.parent, Some(Parent::Contract(_)));
        let public = self.attributes.visibility == Visibility::Public;
        parent_is_contract && public
    }
}

impl Validate for VariableDeclaration {
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
            "@variable [StateVariableDefinition
            @variable_type type_name:[TypeName]
            @variable_attr attributes:[StateVariableAttributes]
            @variable_name name:[Identifier]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let variable = capture(&m, "variable")?;
        let var_type = capture(&m, "variable_type")?;
        let attributes = capture(&m, "variable_attr")?;
        let name = capture(&m, "variable_name")?;

        let span = var_type.text_range().start..variable.text_range().end;
        let name = name.node().unparse().trim().to_string();
        let natspec = extract_comment(variable.clone(), &[])?;
        let parent = extract_parent_name(variable);

        Ok(VariableDeclaration {
            parent,
            name,
            span,
            natspec,
            attributes: extract_attributes(attributes),
        }
        .into())
    }

    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: ItemType::Variable,
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            if options.inheritdoc && self.requires_inheritdoc() {
                out.diags.push(Diagnostic {
                    span: self.span(),
                    message: "@inheritdoc is missing".to_string(),
                });
            } else {
                out.diags.push(Diagnostic {
                    span: self.span(),
                    message: "missing NatSpec".to_string(),
                });
            }
            return out;
        };
        // if there is `inheritdoc`, no further validation is required
        if natspec
            .items
            .iter()
            .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
            return out;
        } else if options.inheritdoc && self.requires_inheritdoc() {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "@inheritdoc is missing".to_string(),
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;
    use similar_asserts::assert_eq;
    use slang_solidity::{cst::NonterminalKind, parser::Parser};

    use super::*;

    static OPTIONS: ValidationOptions = ValidationOptions {
        inheritdoc: true,
        constructor: false,
        struct_params: false,
        enum_params: false,
    };

    fn parse_file(contents: &str) -> VariableDeclaration {
        let parser = Parser::create(Version::new(0, 8, 0)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor
            .query(vec![VariableDeclaration::query()])
            .next()
            .unwrap();
        let def = VariableDeclaration::extract(m).unwrap();
        def.as_variable().unwrap()
    }

    #[test]
    fn test_variable() {
        let contents = "contract Test is ITest {
            /// @inheritdoc ITest
            uint256 public a;
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_variable_no_natspec_inheritdoc() {
        let contents = "contract Test {
            uint256 public a;
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@inheritdoc is missing");
    }

    #[test]
    fn test_variable_only_notice() {
        let contents = "contract Test {
            /// @notice The variable
            uint256 public a;
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@inheritdoc is missing");
    }

    #[test]
    fn test_variable_multiline() {
        let contents = "contract Test is ITest {
            /**
             * @inheritdoc ITest
             */
            uint256 public a;
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_requires_inheritdoc() {
        let contents = "contract Test is ITest {
            uint256 public a;
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc());

        let contents = "contract Test is ITest {
            uint256 internal a;
        }";
        let res = parse_file(contents);
        assert!(!res.requires_inheritdoc());
    }

    #[test]
    fn test_variable_no_inheritdoc() {
        let contents = "contract Test {
            uint256 internal a;
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(false)
                .constructor(false)
                .struct_params(false)
                .enum_params(false)
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
    }
}
