use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, extract_comment, extract_params, parent_contract_name, Definition,
    Identifier, Validate,
};

#[derive(Debug, Clone)]
pub struct ModifierDefinition {
    pub contract: Option<String>,
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for ModifierDefinition {
    fn contract(&self) -> Option<String> {
        self.contract.clone()
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
            ]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let modifier = capture!(m, "modifier");
        let name = capture!(m, "modifier_name");
        let params = capture!(m, "modifier_params");

        let span = params.text_range();
        let name = name.node().unparse().trim().to_string();
        let params = extract_params(params, NonterminalKind::Parameter);
        let natspec = extract_comment(modifier.clone(), &[])?;
        let contract = parent_contract_name(modifier);

        Ok(ModifierDefinition {
            contract,
            name,
            span,
            params,
            natspec,
        }
        .into())
    }

    fn validate(&self) -> Vec<Diagnostic> {
        let mut res = Vec::new();
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                contract: self.contract(),
                item_type: ItemType::Modifier,
                item_name: self.name(),
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
            ItemType::Modifier,
        ));
        res
    }
}
