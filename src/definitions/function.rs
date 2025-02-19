use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
};

use super::{
    capture, check_params, check_returns, extract_attributes, extract_comment, extract_params,
    extract_parent_name, Attributes, Definition, Identifier, Parent, Validate, ValidationOptions,
    Visibility,
};

/// A function definition
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
            @keyword function_keyword:[FunctionKeyword]
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
        let keyword = capture!(m, "keyword");
        let name = capture!(m, "function_name");
        let params = capture!(m, "function_params");
        let attributes = capture!(m, "function_attr");
        let returns = capture!(m, "function_returns");

        let span = keyword.text_range().start..returns.text_range().end;
        let name = name.node().unparse().trim().to_string();
        let params = extract_params(params, NonterminalKind::Parameter);
        let returns = extract_params(returns, NonterminalKind::Parameter);
        let natspec = extract_comment(func.clone(), &returns)?;
        let parent = extract_parent_name(func);

        Ok(FunctionDefinition {
            parent,
            name,
            span,
            params,
            returns,
            natspec,
            attributes: extract_attributes(attributes),
        }
        .into())
    }

    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: ItemType::Function,
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        // fallback and receive do not require NatSpec
        if self.name == "receive" || self.name == "fallback" {
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
        } else if options.inheritdoc && self.requires_inheritdoc() {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "@inheritdoc is missing".to_string(),
            });
            return out;
        }
        // check params and returns
        out.diags.append(&mut check_params(natspec, &self.params));
        out.diags.append(&mut check_returns(natspec, &self.returns));
        out
    }
}
