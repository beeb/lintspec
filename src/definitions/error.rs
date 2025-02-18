use slang_solidity::cst::{Query, QueryMatch, TextRange};

use crate::{
    comment::NatSpec,
    error::Result,
    lint::{CheckType, Diagnostic},
};

use super::{
    capture, check_params, extract_comment, extract_identifiers, Definition, Identifier, Validate,
};

#[derive(Debug, Clone)]
pub struct ErrorDefinition {
    pub name: String,
    pub span: TextRange,
    pub members: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for ErrorDefinition {
    fn query() -> Query {
        Query::parse(
            "@err [ErrorDefinition
            @err_name name:[Identifier]
            @err_members members:[ErrorParametersDeclaration]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let err = capture!(m, "err");
        let name = capture!(m, "err_name");
        let members = capture!(m, "err_members");

        let span = name.text_range();
        let name = name.node().unparse();
        let members = extract_identifiers(members);
        let natspec = extract_comment(err, &[])?;

        Ok(ErrorDefinition {
            name,
            span,
            members,
            natspec,
        }
        .into())
    }

    fn validate(&self) -> Vec<Diagnostic> {
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                check_type: CheckType::Error,
                span: self.span.clone(),
                message: "missing NatSpec".to_string(),
            }];
        };
        check_params(natspec, &self.members, CheckType::Error)
    }
}
