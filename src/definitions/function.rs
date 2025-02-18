use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, check_returns, extract_comment, extract_params, parent_contract_name,
    Definition, Identifier, Validate,
};

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub parent: Option<String>,
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub returns: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for FunctionDefinition {
    fn parent(&self) -> Option<String> {
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
            "@function [FunctionDefinition
            @function_name name:[FunctionName]
            parameters:[ParametersDeclaration
                @function_params parameters:[Parameters]
            ]
            returns:[
                ReturnsDeclaration variables:[ParametersDeclaration
                    @function_returns parameters:[Parameters]
                ]
            ]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let func = capture!(m, "function");
        let name = capture!(m, "function_name");
        let params = capture!(m, "function_params");
        let returns = capture!(m, "function_returns");

        let span = params.text_range();
        let name = name.node().unparse().trim().to_string();
        let params = extract_params(params, NonterminalKind::Parameter);
        let returns = extract_params(returns, NonterminalKind::Parameter);
        let natspec = extract_comment(func.clone(), &returns)?;
        let parent = parent_contract_name(func);

        Ok(FunctionDefinition {
            parent,
            name,
            span,
            params,
            returns,
            natspec,
        }
        .into())
    }

    fn validate(&self) -> Vec<Diagnostic> {
        let mut res = Vec::new();
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                parent: self.parent(),
                item_type: ItemType::Function,
                item_name: self.name(),
                span: self.span(),
                message: "missing NatSpec".to_string(),
            }];
        };
        // fallback and receive do not require NatSpec
        if self.name == "receive" || self.name == "fallback" {
            return vec![];
        }
        // if there is `inheritdoc`, no further validation is required
        if natspec
            .items
            .iter()
            .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
            return vec![];
        }
        // check params and returns
        res.append(&mut check_params(
            self,
            natspec,
            &self.params,
            ItemType::Function,
        ));
        res.append(&mut check_returns(
            self,
            natspec,
            &self.returns,
            ItemType::Function,
        ));
        res
    }
}
