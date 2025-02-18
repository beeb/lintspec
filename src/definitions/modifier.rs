use slang_solidity::cst::{NonterminalKind, Query, QueryMatch, TextRange};

use crate::{
    comment::{NatSpec, NatSpecKind},
    error::Result,
    lint::{CheckType, Diagnostic},
};

use super::{
    capture, check_params, check_returns, extract_comment, extract_params, Definition, Identifier,
    Validate,
};

#[derive(Debug, Clone)]
pub struct ModifierDefinition {
    pub name: String,
    pub span: TextRange,
    pub params: Vec<Identifier>,
    pub natspec: Option<NatSpec>,
}

impl Validate for ModifierDefinition {
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

        let span = name.text_range();
        let name = name.node().unparse();
        let params = extract_params(params, NonterminalKind::Parameter);
        let natspec = extract_comment(modifier, &[])?;

        Ok(ModifierDefinition {
            name,
            span,
            params,
            natspec,
        }
        .into())
    }

    fn validate(&self) -> Vec<Diagnostic> {
        let mut res = Vec::new();
        // raise error if no NatSpec is available
        let Some(natspec) = &self.natspec else {
            return vec![Diagnostic {
                check_type: CheckType::Modifier,
                span: self.span.clone(),
                message: "missing NatSpec".to_string(),
            }];
        };
        // if there is `inheritdoc`, no further validation is required
        if natspec
            .items
            .iter()
            .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
            return vec![];
        }
        // check params
        res.append(&mut check_params(
            natspec,
            &self.params,
            CheckType::Modifier,
        ));
        res
    }
}
