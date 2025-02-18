use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, check_returns, extract_comment, extract_params, Definition, Identifier,
    Validate,
};

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub returns: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for FunctionDefinition {
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

        let span = name.text_range();
        let name = name.node().unparse().trim().to_string();
        let params = extract_params(params, NonterminalKind::Parameter);
        let returns = extract_params(returns, NonterminalKind::Parameter);
        let natspec = extract_comment(func, &returns)?;

        Ok(FunctionDefinition {
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
                item_type: ItemType::Function,
                item_name: self.name.clone(),
                span: self.span.clone(),
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
            &self.name,
            natspec,
            &self.params,
            ItemType::Function,
        ));
        res.append(&mut check_returns(
            &self.name,
            natspec,
            &self.returns,
            ItemType::Function,
        ));
        res
    }
}
