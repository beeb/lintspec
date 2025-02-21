use slang_solidity::cst::{Cursor, Query, QueryMatch, TerminalKind, TextRange};

use crate::{
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
    natspec::NatSpec,
};

use super::{
    capture, check_params, extract_comment, extract_parent_name, Definition, Identifier, Parent,
    Validate, ValidationOptions,
};

/// An enum definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct EnumDefinition {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub members: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for EnumDefinition {
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
            "@enum [EnumDefinition
            @enum_name name:[Identifier]
            @enum_members members:[EnumMembers]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let enumeration = capture(&m, "enum")?;
        let name = capture(&m, "enum_name")?;
        let members = capture(&m, "enum_members")?;

        let span = enumeration.text_range();
        let name = name.node().unparse().trim().to_string();
        let members = extract_enum_members(members);
        let natspec = extract_comment(enumeration.clone(), &[])?;
        let parent = extract_parent_name(enumeration);

        Ok(EnumDefinition {
            parent,
            name,
            span,
            members,
            natspec,
        }
        .into())
    }

    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: ItemType::Enum,
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
        if options.enum_params {
            out.diags = check_params(natspec, &self.members);
        }
        out
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
