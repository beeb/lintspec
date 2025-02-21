use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
    natspec::{NatSpec, NatSpecKind},
};

use super::{
    capture, capture_opt, check_params, extract_attributes, extract_comment, extract_params,
    extract_parent_name, Attributes, Definition, Identifier, Parent, Validate, ValidationOptions,
};

/// A modifier definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct ModifierDefinition {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
    pub attributes: Attributes,
}

impl ModifierDefinition {
    /// Check whether this modifier requires inheritdoc when we enforce it
    ///
    /// Overridden modifiers must have inheritdoc.
    fn requires_inheritdoc(&self) -> bool {
        let parent_is_contract = matches!(self.parent, Some(Parent::Contract(_)));
        parent_is_contract && self.attributes.r#override
    }
}

impl Validate for ModifierDefinition {
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
            "@modifier [ModifierDefinition
            @modifier_name name:[Identifier]
            parameters:[ParametersDeclaration
                @modifier_params parameters:[Parameters]
            ]?
            @modifier_attr attributes:[ModifierAttributes]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let modifier = capture(&m, "modifier")?;
        let name = capture(&m, "modifier_name")?;
        let params = capture_opt(&m, "modifier_params")?;
        let attr = capture(&m, "modifier_attr")?;

        let span = if let Some(params) = &params {
            name.text_range().start..params.text_range().end
        } else {
            name.text_range().start..attr.text_range().end
        };
        let name = name.node().unparse().trim().to_string();
        let params = params
            .map(|p| extract_params(p, NonterminalKind::Parameter))
            .unwrap_or_default();

        let natspec = extract_comment(modifier.clone(), &[])?;
        let parent = extract_parent_name(modifier);

        Ok(ModifierDefinition {
            parent,
            name,
            span,
            params,
            natspec,
            attributes: extract_attributes(attr),
        }
        .into())
    }

    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: ItemType::Modifier,
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        // raise error if no NatSpec is available (unless there are no params and we don't enforce inheritdoc)
        let Some(natspec) = &self.natspec else {
            if self.params.is_empty() && (!options.inheritdoc || !self.requires_inheritdoc()) {
                return out;
            }
            // we require natspec for either inheritdoc or the params
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "missing NatSpec".to_string(),
            });
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
            return out;
        }
        // check params and returns
        out.diags.append(&mut check_params(natspec, &self.params));
        out
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;
    use slang_solidity::parser::Parser;

    use super::*;

    static OPTIONS: ValidationOptions = ValidationOptions {
        inheritdoc: false,
        constructor: false,
        struct_params: false,
        enum_params: false,
    };

    fn parse_file(contents: &str) -> ModifierDefinition {
        let parser = Parser::create(Version::new(0, 8, 0)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor
            .query(vec![ModifierDefinition::query()])
            .next()
            .unwrap();
        let def = ModifierDefinition::extract(m).unwrap();
        def.as_modifier().unwrap()
    }

    #[test]
    fn test_modifier() {
        let contents = "contract Test {
            /// @param param1 Test
            /// @param param2 Test2
            modifier foo(uint256 param1, bytes calldata param2) { _; } 
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_no_natspec() {
        let contents = "contract Test {
            modifier foo(uint256 param1, bytes calldata param2) { _; } 
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
    }

    #[test]
    fn test_modifier_only_notice() {
        let contents = "contract Test {
            /// @notice The modifier
            modifier foo(uint256 param1, bytes calldata param2) { _; } 
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param param1 is missing");
        assert_eq!(res.diags[1].message, "@param param2 is missing");
    }

    #[test]
    fn test_modifier_one_missing() {
        let contents = "contract Test {
            /// @param param1 The first
            modifier foo(uint256 param1, bytes calldata param2) { _; } 
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param param2 is missing");
    }

    #[test]
    fn test_modifier_multiline() {
        let contents = "contract Test {
            /**
             * @param param1 Test
             * @param param2 Test2
             */
            modifier foo(uint256 param1, bytes calldata param2) { _; } 
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_duplicate() {
        let contents = "contract Test {
            /// @param param1 The first
            /// @param param1 The first again
            modifier foo(uint256 param1) { _; } 
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(
            res.diags[0].message,
            "@param param1 is present more than once"
        );
    }

    #[test]
    fn test_modifier_no_params() {
        let contents = "contract Test {
            modifier foo()  { _; } 
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_no_params_no_paren() {
        let contents = "contract Test {
            modifier foo { _; } 
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_requires_inheritdoc() {
        let contents = "contract Test is ITest {
            modifier a() { _; } 
        }";
        let res = parse_file(contents);
        assert!(!res.requires_inheritdoc());

        let contents = "contract Test is ITest {
            modifier e() override (ITest) { _; } 
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc());
    }

    #[test]
    fn test_modifier_inheritdoc() {
        let contents = "contract Test is ITest {
            /// @inheritdoc ITest
            modifier foo() override (ITest) { _; } 
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(true)
                .constructor(false)
                .struct_params(false)
                .enum_params(false)
                .build(),
        );
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_inheritdoc_missing() {
        let contents = "contract Test is ITest {
            /// @notice Test
            modifier foo() override (ITest) { _; } 
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc(true)
                .constructor(false)
                .struct_params(false)
                .enum_params(false)
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@inheritdoc is missing");
    }
}
