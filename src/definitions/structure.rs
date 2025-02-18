use slang_solidity::cst::{Cursor, Query, QueryMatch, TextRange};

use crate::{
    comment::NatSpec,
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, extract_comment, parent_contract_name, Definition, Identifier, Validate,
};

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub contract: Option<String>,
    pub name: String,
    pub span: TextRange,
    pub members: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for StructDefinition {
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

        let span = structure.text_range();
        let name = name.node().unparse().trim().to_string();
        let members = extract_struct_members(members)?;
        let natspec = extract_comment(structure.clone(), &[])?;
        let contract = parent_contract_name(structure);

        Ok(StructDefinition {
            contract,
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
                contract: self.contract(),
                item_type: ItemType::Struct,
                item_name: self.name(),
                span: self.span(),
                message: "missing NatSpec".to_string(),
            }];
        };
        check_params(self, natspec, &self.members, ItemType::Struct)
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
