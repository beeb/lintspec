use std::path::Path;

use slang_solidity::cst::{Query, QueryMatch};

use crate::{comment::NatSpec, error::Result, lint::Diagnostic};

use super::{capture, extract_comment, extract_identifiers, Definition, Identifier, Validate};

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub name: String,
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
        let name = name.node().unparse();
        let members = extract_identifiers(members);
        let natspec = extract_comment(structure, &[])?;

        Ok(StructDefinition {
            name,
            members,
            natspec,
        }
        .into())
    }

    fn validate(&self, file_path: impl AsRef<Path>) -> Vec<Diagnostic> {
        let path = file_path.as_ref().to_path_buf();
        let mut res = Vec::new();
        // TODO
        res
    }
}
