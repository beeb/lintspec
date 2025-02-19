use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    natspec::NatSpec,
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
};

use super::{
    capture, check_params, extract_comment, extract_params, extract_parent_name, Definition,
    Identifier, Parent, Validate, ValidationOptions,
};

/// An event definition
#[derive(Debug, Clone)]
pub struct EventDefinition {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for EventDefinition {
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
        let parent = extract_parent_name(event);

        Ok(EventDefinition {
            parent,
            name,
            span,
            params,
            natspec,
        }
        .into())
    }

    fn validate(&self, _: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: ItemType::Event,
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "missing NatSpec".to_string(),
            });
            return out;
        };
        out.diags = check_params(natspec, &self.params);
        out
    }
}
