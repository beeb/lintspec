use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::NatSpec,
    error::Result,
    lint::{CheckType, Diagnostic},
};

use super::{
    capture, check_params, extract_comment, extract_params, Definition, Identifier, Validate,
};

#[derive(Debug, Clone)]
pub struct EventDefinition {
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for EventDefinition {
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

        let span = name.text_range();
        let name = name.node().unparse();
        let params = extract_params(params, NonterminalKind::EventParameter);
        let natspec = extract_comment(event, &[])?;

        Ok(EventDefinition {
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
                check_type: CheckType::Event,
                span: self.span.clone(),
                message: "missing NatSpec".to_string(),
            }];
        };
        check_params(natspec, &self.params, CheckType::Event)
    }
}
