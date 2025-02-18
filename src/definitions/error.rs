use slang_solidity::cst::{Query, QueryMatch, TextRange};

use crate::{
    comment::NatSpec,
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, extract_comment, extract_identifiers, Definition, Identifier, Validate,
};

#[derive(Debug, Clone)]
pub struct ErrorDefinition {
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for ErrorDefinition {
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

        let span = name.text_range();
        let name = name.node().unparse().trim().to_string();
        let params = extract_identifiers(params);
        let natspec = extract_comment(err, &[])?;

        Ok(ErrorDefinition {
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
                item_type: ItemType::Error,
                item_name: self.name.clone(),
                span: self.span.clone(),
                message: "missing NatSpec".to_string(),
            }];
        };
        check_params(&self.name, natspec, &self.params, ItemType::Error)
    }
}
