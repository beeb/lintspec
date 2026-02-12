//! Parsing and validation of contract definitions.
use crate::{
    interner::Symbol,
    lint::{CheckAuthor, CheckNoticeAndDev, CheckTitle, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions};

/// A contract definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct ContractDefinition {
    /// The name of the contract
    pub name: Symbol,

    /// The span of the contract definition
    pub span: TextRange,

    /// The [`NatSpec`] associated with the contract definition, if any
    pub natspec: Option<NatSpec>,
}

impl SourceItem for ContractDefinition {
    fn item_type(&self) -> ItemType {
        ItemType::Contract
    }

    fn parent(&self) -> Option<Parent> {
        None
    }

    fn name(&self) -> Symbol {
        self.name
    }

    fn span(&self) -> TextRange {
        self.span.clone()
    }
}

impl Validate for ContractDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let opts = &options.contracts;
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: self.item_type(),
            name: self.name().resolve(),
            span: self.span(),
            diags: vec![],
        };
        CheckTitle::builder()
            .natspec(&self.natspec)
            .rule(opts.title)
            .span(&self.span)
            .build()
            .check_into(&mut out.diags);
        CheckAuthor::builder()
            .natspec(&self.natspec)
            .rule(opts.author)
            .span(&self.span)
            .build()
            .check_into(&mut out.diags);
        CheckNoticeAndDev::builder()
            .natspec(&self.natspec)
            .notice_rule(opts.notice)
            .dev_rule(opts.dev)
            .notice_or_dev(options.notice_or_dev)
            .span(&self.span)
            .build()
            .check_into(&mut out.diags);
        out
    }
}

#[cfg(test)]
#[cfg(feature = "solar")]
mod tests {
    use std::sync::LazyLock;

    use similar_asserts::assert_eq;

    use crate::{
        config::{ContractRules, Req},
        definitions::Definition,
        parser::{Parse as _, solar::SolarParser},
    };

    use super::*;

    static OPTIONS: LazyLock<ValidationOptions> = LazyLock::new(|| {
        ValidationOptions::builder()
            .inheritdoc(false)
            .contracts(
                ContractRules::builder()
                    .title(Req::Required)
                    .author(Req::Required)
                    .notice(Req::Required)
                    .build(),
            )
            .build()
    });

    fn parse_file(contents: &str) -> ContractDefinition {
        let mut parser = SolarParser::default();
        let doc = parser
            .parse_document(contents.as_bytes(), None::<std::path::PathBuf>, false)
            .unwrap();
        doc.definitions
            .into_iter()
            .find_map(Definition::to_contract)
            .unwrap()
    }

    #[test]
    fn test_contract() {
        let contents = "/// @title Contract
        /// @author Me
        /// @notice This is a contract
        contract Test {}";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_contract_no_natspec() {
        let contents = "contract Test {}";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 3);
        assert_eq!(res.diags[0].message, "@title is missing");
        assert_eq!(res.diags[1].message, "@author is missing");
        assert_eq!(res.diags[2].message, "@notice is missing");
    }

    #[test]
    fn test_contract_multiline() {
        let contents = "/**
         * @title Contract
         * @author Me
         * @notice This is a contract
         */
        contract Test {}";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_contract_inheritdoc() {
        // inheritdoc should be ignored as it doesn't apply to contracts
        let contents = "/// @inheritdoc ITest
        contract Test is Foo {}";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .contracts(ContractRules::builder().title(Req::Required).build())
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@title is missing");
    }
}
