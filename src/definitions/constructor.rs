use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
    natspec::{NatSpec, NatSpecKind},
};

use super::{
    capture, check_params, extract_comment, extract_params, extract_parent_name, Definition,
    Identifier, Parent, Validate, ValidationOptions,
};

/// A constructor definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
pub struct ConstructorDefinition {
    pub parent: Option<Parent>,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for ConstructorDefinition {
    fn parent(&self) -> Option<Parent> {
        self.parent.clone()
    }

    fn name(&self) -> String {
        "constructor".to_string()
    }

    fn span(&self) -> TextRange {
        self.span.clone()
    }

    fn query() -> Query {
        Query::parse(
            "@constructor [ConstructorDefinition
            parameters:[ParametersDeclaration
                @constructor_params parameters:[Parameters]
            ]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let constructor = capture(&m, "constructor")?;
        let params = capture(&m, "constructor_params")?;

        let span = params.text_range();
        let params = extract_params(params, NonterminalKind::Parameter);
        let natspec = extract_comment(constructor.clone(), &[])?;
        let parent = extract_parent_name(constructor);

        Ok(ConstructorDefinition {
            parent,
            span,
            params,
            natspec,
        }
        .into())
    }

    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: ItemType::Constructor,
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        if !options.constructor {
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
        // if there is `inheritdoc`, no further validation is required
        if natspec
            .items
            .iter()
            .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
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
    use slang_solidity::parser::Parser;

    use super::*;

    static OPTIONS: ValidationOptions = ValidationOptions {
        inheritdoc: false,
        constructor: true,
        enum_params: false,
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
        def.as_constructor().unwrap()
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
}
