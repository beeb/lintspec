use slang_solidity::cst::{Query, QueryMatch, TextRange};

use crate::{
    comment::NatSpec,
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, extract_comment, extract_identifiers, parent_contract_name, Definition,
    Identifier, Parent, Validate, ValidationOptions,
};

#[derive(Debug, Clone)]
pub struct ErrorDefinition {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for ErrorDefinition {
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
            "@err [ErrorDefinition
            @err_name name:[Identifier]
            @err_params members:[ErrorParametersDeclaration]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let err = capture!(m, "err");
        let name = capture!(m, "err_name");
        let params = capture!(m, "err_params");

        let span = err.text_range();
        let name = name.node().unparse().trim().to_string();
        let params = extract_identifiers(params);
        let natspec = extract_comment(err.clone(), &[])?;
        let parent = parent_contract_name(err);

        Ok(ErrorDefinition {
            parent,
            name,
            span,
            params,
            natspec,
        }
        .into())
    }

    fn validate(&self, _: &ValidationOptions) -> Vec<Diagnostic> {
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                parent: self.parent.clone(),
                item_type: ItemType::Error,
                item_name: self.name.clone(),
                item_span: self.span(),
                span: self.span(),
                message: "missing NatSpec".to_string(),
            }];
        };
        check_params(self, natspec, &self.params, ItemType::Error)
    }
}
