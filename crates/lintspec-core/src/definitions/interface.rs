//! Parsing and validation of interface definitions.
use crate::{
    interner::{INTERNER, Symbol},
    lint::{CheckAuthor, CheckNoticeAndDev, CheckTitle, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions};

/// An interface definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct InterfaceDefinition {
    /// The name of the interface
    pub name: Symbol,

    /// The span of the interface definition
    pub span: TextRange,

    /// The [`NatSpec`] associated with the interface definition, if any
    pub natspec: Option<NatSpec>,
}

impl SourceItem for InterfaceDefinition {
    fn item_type(&self) -> ItemType {
        ItemType::Interface
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

impl Validate for InterfaceDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let opts = &options.interfaces;
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: self.item_type(),
            name: self.name().resolve_with(&INTERNER),
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
            .interfaces(
                ContractRules::builder()
                    .title(Req::Required)
                    .author(Req::Required)
                    .notice(Req::Required)
                    .build(),
            )
            .build()
    });

    fn parse_file(contents: &str) -> InterfaceDefinition {
        let mut parser = SolarParser::default();
        let doc = parser
            .parse_document(contents.as_bytes(), None::<std::path::PathBuf>, false)
            .unwrap();
        doc.definitions
            .into_iter()
            .find_map(Definition::to_interface)
            .unwrap()
    }

    #[test]
    fn test_interface() {
        let contents = "/// @title Interface
        /// @author Me
        /// @notice This is an interface
        interface Test {}";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_interface_no_natspec() {
        let contents = "interface Test {}";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 3);
        assert_eq!(res.diags[0].message, "@title is missing");
        assert_eq!(res.diags[1].message, "@author is missing");
        assert_eq!(res.diags[2].message, "@notice is missing");
    }

    #[test]
    fn test_interface_multiline() {
        let contents = "/**
         * @title Interface
         * @author Me
         * @notice This is an interface
         */
        interface Test {}";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_interface_inheritdoc() {
        // inheritdoc should be ignored as it doesn't apply to interfaces
        let contents = "/// @inheritdoc ITest
        interface Test is Foo {}";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .interfaces(ContractRules::builder().title(Req::Required).build())
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@title is missing");
    }
}
