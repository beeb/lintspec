use slang_solidity::cst::{Query, QueryMatch, TextRange};

use crate::{
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
    natspec::{NatSpec, NatSpecKind},
};

use super::{
    capture, extract_attributes, extract_comment, extract_parent_name, Attributes, Definition,
    Parent, Validate, ValidationOptions, Visibility,
};

/// A state variable declaration
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
            @variable_type type_name:[TypeName]
            @variable_attr attributes:[StateVariableAttributes]
            @variable_name name:[Identifier]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let variable = capture(&m, "variable")?;
        let var_type = capture(&m, "variable_type")?;
        let attributes = capture(&m, "variable_attr")?;
        let name = capture(&m, "variable_name")?;

        let span = var_type.text_range().start..variable.text_range().end;
        let name = name.node().unparse().trim().to_string();
        let natspec = extract_comment(variable.clone(), &[])?;
        let parent = extract_parent_name(variable);

        Ok(VariableDeclaration {
            parent,
            name,
            span,
            natspec,
            attributes: extract_attributes(attributes),
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
