//! Parsing and validation of interface definitions.
use crate::{
    lint::{ItemDiagnostics, check_author, check_notice_and_dev, check_title},
    natspec::NatSpec,
};

use super::{ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions};

/// An interface definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct InterfaceDefinition {
    /// The name of the interface
    pub name: String,

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

    fn name(&self) -> String {
        self.name.clone()
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
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        out.diags
            .extend(check_title(&self.natspec, opts.title, self.span()));
        out.diags
            .extend(check_author(&self.natspec, opts.author, self.span()));
        out.diags.extend(check_notice_and_dev(
            &self.natspec,
            opts.notice,
            opts.dev,
            options.notice_or_dev,
            self.span(),
        ));
        out
    }
}

#[cfg(all(test, feature = "slang"))]
mod tests {
    use std::sync::LazyLock;

    use similar_asserts::assert_eq;

    use crate::{
        config::{ContractRules, Req},
        definitions::Definition,
        parser::{Parse as _, slang::SlangParser},
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
        let mut parser = SlangParser::builder().skip_version_detection(true).build();
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
