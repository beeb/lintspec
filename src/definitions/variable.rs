use slang_solidity::cst::{Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{capture, extract_comment, Definition, Validate};

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub name: String,
    pub span: TextRange,
    pub natspec: Option<NatSpec>,
}

impl Validate for VariableDeclaration {
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

        let span = name.text_range();
        let name = name.node().unparse();
        let natspec = extract_comment(variable, &[])?;

        Ok(VariableDeclaration {
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
                item_type: ItemType::Variable,
                item_name: self.name.clone(),
                span: self.span.clone(),
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
                item_type: ItemType::Variable,
                item_name: self.name.clone(),
                span: self.span.clone(),
                message: "missing @notice or @dev".to_string(),
            }];
        }
        vec![]
    }
}
