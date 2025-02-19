use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{Diagnostic, ItemDiagnostics, ItemType},
};

use super::{
    capture, check_params, extract_comment, extract_params, parent_contract_name, Definition,
    Identifier, Parent, Validate, ValidationOptions,
};

#[derive(Debug, Clone)]
pub struct ModifierDefinition {
    pub parent: Option<Parent>,
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for ModifierDefinition {
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
            "@modifier [ModifierDefinition
            @modifier_name name:[Identifier]
            parameters:[ParametersDeclaration
                @modifier_params parameters:[Parameters]
            ]
        ]",
        )
        .expect("query should compile")
    }

    fn extract(m: QueryMatch) -> Result<Definition> {
        let modifier = capture!(m, "modifier");
        let name = capture!(m, "modifier_name");
        let params = capture!(m, "modifier_params");

        let span = name.text_range().start..params.text_range().end;
        let name = name.node().unparse().trim().to_string();
        let params = extract_params(params, NonterminalKind::Parameter);
        let natspec = extract_comment(modifier.clone(), &[])?;
        let parent = parent_contract_name(modifier);

        Ok(ModifierDefinition {
            parent,
            name,
            span,
            params,
            natspec,
        }
        .into())
    }

    fn validate(&self, _: &ValidationOptions) -> ItemDiagnostics {
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: ItemType::Modifier,
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
        }
        // check params
        out.diags.append(&mut check_params(natspec, &self.params));
        out
    }
}
