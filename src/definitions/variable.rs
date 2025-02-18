use slang_solidity::cst::{Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{capture, extract_comment, parent_contract_name, Definition, Validate};

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub contract: Option<String>,
    pub name: String,
    pub span: TextRange,
    pub natspec: Option<NatSpec>,
}

impl Validate for VariableDeclaration {
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
            "@variable [StateVariableDefinition
            @variable_name name:[Identifier]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let variable = capture!(m, "variable");
        let name = capture!(m, "variable_name");

        let span = variable.text_range();
        let name = name.node().unparse().trim().to_string();
        let natspec = extract_comment(variable.clone(), &[])?;
        let contract = parent_contract_name(variable);

        Ok(VariableDeclaration {
            contract,
            name,
            span,
            natspec,
        }
        .into())
    }

    fn validate(&self) -> Vec<Diagnostic> {
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                contract: self.contract(),
                item_type: ItemType::Variable,
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
        if !natspec
            .items
            .iter()
            .any(|n| matches!(n.kind, NatSpecKind::Notice | NatSpecKind::Dev))
        {
            return vec![Diagnostic {
                contract: self.contract(),
                item_type: ItemType::Variable,
                item_name: self.name(),
                span: self.span(),
                message: "missing @notice or @dev".to_string(),
            }];
        }
        vec![]
    }
}
