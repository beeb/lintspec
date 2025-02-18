use slang_solidity::cst::{Query, QueryMatch, TextRange};

use crate::{
    comment::NatSpec,
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, extract_comment, extract_identifiers, parent_contract_name, Definition,
    Identifier, Validate,
};

#[derive(Debug, Clone)]
pub struct ErrorDefinition {
    pub contract: Option<String>,
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for ErrorDefinition {
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
        let contract = parent_contract_name(err);

        Ok(ErrorDefinition {
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
                contract: self.contract.clone(),
                item_type: ItemType::Error,
                item_name: self.name.clone(),
                span: self.span(),
                message: "missing NatSpec".to_string(),
            }];
        };
        check_params(self, natspec, &self.params, ItemType::Error)
    }
}
