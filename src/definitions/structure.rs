use slang_solidity::cst::{Cursor, Query, QueryMatch, TextRange};

use crate::{
    comment::NatSpec,
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{capture, check_params, extract_comment, Definition, Identifier, Validate};

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
        let name = name.node().unparse().trim().to_string();
        let members = extract_struct_members(members)?;
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
                item_type: ItemType::Struct,
                item_name: self.name.clone(),
                span: self.span.clone(),
                message: "missing NatSpec".to_string(),
            }];
        };
        check_params(&self.name, natspec, &self.members, ItemType::Struct)
    }
}

fn extract_struct_members(cursor: Cursor) -> Result<Vec<Identifier>> {
    let cursor = cursor.spawn();
    let mut out = Vec::new();
    let query = Query::parse(
        "[StructMember
        @member_name name:[Identifier]
    ]",
    )
    .expect("query should compile");
    for m in cursor.query(vec![query]) {
        let member_name = capture!(m, "member_name");
        out.push(Identifier {
            name: Some(member_name.node().unparse().trim().to_string()),
            span: member_name.text_range(),
        })
    }
    Ok(out)
}
