use slang_solidity::cst::{Cursor, Query, QueryMatch, TerminalKind, TextRange};

use crate::{
    comment::NatSpec,
    error::Result,
    lint::{Diagnostic, ItemType},
};

use super::{
    capture, check_params, extract_comment, parent_contract_name, Definition, Identifier, Validate,
};

#[derive(Debug, Clone)]
pub struct EnumDefinition {
    pub parent: Option<String>,
    pub name: String,
    pub span: TextRange,
    pub members: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
    pub check_params: bool,
}

impl Validate for EnumDefinition {
    fn parent(&self) -> Option<String> {
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
            "@enum [EnumDefinition
            @enum_name name:[Identifier]
            @enum_members members:[EnumMembers]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let enumeration = capture!(m, "enum");
        let name = capture!(m, "enum_name");
        let members = capture!(m, "enum_members");

        let span = enumeration.text_range();
        let name = name.node().unparse().trim().to_string();
        let members = extract_enum_members(members);
        let natspec = extract_comment(enumeration.clone(), &[])?;
        let parent = parent_contract_name(enumeration);

        Ok(EnumDefinition {
            parent,
            name,
            span,
            members,
            natspec,
            check_params: false,
        }
        .into())
    }

    fn validate(&self) -> Vec<Diagnostic> {
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                parent: self.parent(),
                item_type: ItemType::Enum,
                item_name: self.name(),
                item_span: self.span(),
                span: self.span(),
                message: "missing NatSpec".to_string(),
            }];
        };
        if self.check_params {
            check_params(self, natspec, &self.members, ItemType::Enum)
        } else {
            vec![]
        }
    }
}

fn extract_enum_members(cursor: Cursor) -> Vec<Identifier> {
    let mut cursor = cursor.spawn().with_edges();
    let mut out = Vec::new();
    while cursor.go_to_next_terminal_with_kind(TerminalKind::Identifier) {
        out.push(Identifier {
            name: Some(cursor.node().unparse().trim().to_string()),
            span: cursor.text_range(),
        })
    }
    out
}
