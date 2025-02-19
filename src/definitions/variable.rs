use slang_solidity::cst::{Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
};

use super::{
    capture, extract_comment, get_attributes, parent_contract_name, Attributes, Definition, Parent,
    Validate, ValidationOptions, Visibility,
};

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub natspec: Option<NatSpec>,
    pub attributes: Attributes,
}

impl VariableDeclaration {
    fn requires_inheritdoc(&self) -> bool {
        let parent_is_contract = matches!(self.parent, Some(Parent::Contract(_)));
        let public = self.attributes.visibility == Visibility::Public;
        parent_is_contract && public
    }
}

impl Validate for VariableDeclaration {
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
            "@variable [StateVariableDefinition
            @variable_attr attributes:[StateVariableAttributes]
            @variable_name name:[Identifier]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let variable = capture!(m, "variable");
        let attributes = capture!(m, "variable_attr");
        let name = capture!(m, "variable_name");

        let span = variable.text_range();
        let name = name.node().unparse().trim().to_string();
        let natspec = extract_comment(variable.clone(), &[])?;
        let parent = parent_contract_name(variable);

        Ok(VariableDeclaration {
            parent,
            name,
            span,
            natspec,
            attributes: get_attributes(attributes),
        }
        .into())
    }

    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: ItemType::Variable,
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
        // if there is `inheritdoc`, no further validation is required
        if natspec
            .items
            .iter()
            .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
            return out;
        } else if options.inheritdoc && self.requires_inheritdoc() {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "@inheritdoc is missing".to_string(),
            });
            return out;
        }
        if !natspec
            .items
            .iter()
            .any(|n| matches!(n.kind, NatSpecKind::Notice | NatSpecKind::Dev))
        {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "missing @notice or @dev".to_string(),
            });
        }
        out
    }
}
