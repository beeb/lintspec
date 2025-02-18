use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, extract_comment, extract_params, parent_contract_name, Definition,
    Identifier, Parent, Validate, ValidationOptions,
};

#[derive(Debug, Clone)]
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
        let constructor = capture!(m, "constructor");
        let params = capture!(m, "constructor_params");

        let span = params.text_range();
        let params = extract_params(params, NonterminalKind::Parameter);
        let natspec = extract_comment(constructor.clone(), &[])?;
        let parent = parent_contract_name(constructor);

        Ok(ConstructorDefinition {
            parent,
            span,
            params,
            natspec,
        }
        .into())
    }

    fn validate(&self, options: &ValidationOptions) -> Vec<Diagnostic> {
        if !options.constructor {
            return vec![];
        }
        let mut res = Vec::new();
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                parent: self.parent(),
                item_type: ItemType::Constructor,
                item_name: self.name(),
                item_span: self.span(),
                span: self.span(),
                message: "missing NatSpec".to_string(),
            }];
        };
        // if there is `inheritdoc`, no further validation is required
        if natspec
            .items
            .iter()
            .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
            return vec![];
        }
        // check params
        res.append(&mut check_params(
            self,
            natspec,
            &self.params,
            ItemType::Function,
        ));
        res
    }
}
