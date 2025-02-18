use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, check_returns, extract_comment, extract_params, get_attributes,
    parent_contract_name, Attributes, Definition, Identifier, Parent, Validate, ValidationOptions,
    Visibility,
};

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub returns: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
    pub attributes: Attributes,
}

impl FunctionDefinition {
    fn requires_inheritdoc(&self) -> bool {
        let parent_is_contract = matches!(self.parent, Some(Parent::Contract(_)));
        let internal_override =
            self.attributes.visibility == Visibility::Internal && self.attributes.r#override;
        let public_external = matches!(
            self.attributes.visibility,
            Visibility::External | Visibility::Public
        );
        parent_is_contract && (internal_override || public_external)
    }
}

impl Validate for FunctionDefinition {
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
            "@function [FunctionDefinition
            @function_name name:[FunctionName]
            parameters:[ParametersDeclaration
                @function_params parameters:[Parameters]
            ]
            @function_attr attributes:[FunctionAttributes]
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
        let attributes = capture!(m, "function_attr");
        let returns = capture!(m, "function_returns");

        let span = name.text_range().start..returns.text_range().end;
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
            attributes: get_attributes(attributes),
        }
        .into())
    }

    fn validate(&self, options: &ValidationOptions) -> Vec<Diagnostic> {
        // fallback and receive do not require NatSpec
        if self.name == "receive" || self.name == "fallback" {
            return vec![];
        }
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                parent: self.parent(),
                item_type: ItemType::Function,
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
        } else if options.inheritdoc && self.requires_inheritdoc() {
            return vec![Diagnostic {
                parent: self.parent(),
                item_type: ItemType::Function,
                item_name: self.name(),
                item_span: self.span(),
                span: self.span(),
                message: "@inheritdoc is missing".to_string(),
            }];
        }
        // check params and returns
        let mut res = Vec::new();
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
