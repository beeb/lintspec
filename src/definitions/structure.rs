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
pub struct StructDefinition {
    pub name: String,
    pub span: TextRange,
    pub members: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for StructDefinition {
    fn query() -> Query {
        Query::parse(
            "@struct [StructDefinition
            @struct_name name:[Identifier]
            @struct_members members:[StructMembers]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let structure = capture!(m, "struct");
        let name = capture!(m, "struct_name");
        let members = capture!(m, "struct_members");

        let span = name.text_range();
        let name = name.node().unparse();
        let members = extract_identifiers(members);
        let natspec = extract_comment(structure, &[])?;

        Ok(StructDefinition {
            name,
            span,
            members,
            natspec,
        }
        .into())
    }

    fn validate(&self) -> Vec<Diagnostic> {
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                check_type: CheckType::Struct,
                span: self.span.clone(),
                message: "missing NatSpec".to_string(),
            }];
        };
        check_params(natspec, &self.members, CheckType::Struct)
    }
}
