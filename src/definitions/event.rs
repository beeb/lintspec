use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::NatSpec,
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, extract_comment, extract_params, parent_contract_name, Definition,
    Identifier, Validate,
};

#[derive(Debug, Clone)]
pub struct EventDefinition {
    pub contract: Option<String>,
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for EventDefinition {
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
            "@event [EventDefinition
            @event_name name:[Identifier]
            @event_params parameters:[EventParametersDeclaration]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let event = capture!(m, "event");
        let name = capture!(m, "event_name");
        let params = capture!(m, "event_params");

        let span = event.text_range();
        let name = name.node().unparse().trim().to_string();
        let params = extract_params(params, NonterminalKind::EventParameter);
        let natspec = extract_comment(event.clone(), &[])?;
        let contract = parent_contract_name(event);

        Ok(EventDefinition {
            contract,
            name,
            span,
            params,
            natspec,
        }
        .into())
    }

    fn validate(&self) -> Vec<Diagnostic> {
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                contract: self.contract(),
                item_type: ItemType::Event,
                item_name: self.name(),
                span: self.span(),
                message: "missing NatSpec".to_string(),
            }];
        };
        check_params(self, natspec, &self.params, ItemType::Event)
    }
}
