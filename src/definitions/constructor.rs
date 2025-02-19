use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
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
