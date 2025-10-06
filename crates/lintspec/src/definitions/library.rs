//! Parsing and validation of library definitions.
use crate::{
    lint::{ItemDiagnostics, check_author, check_notice_and_dev, check_title},
    natspec::NatSpec,
};

use super::{ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions};

/// A library definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct LibraryDefinition {
    /// The name of the library
    pub name: String,

    /// The span of the library definition
    pub span: TextRange,

    /// The [`NatSpec`] associated with the library definition, if any
    pub natspec: Option<NatSpec>,
}

impl SourceItem for LibraryDefinition {
    fn item_type(&self) -> ItemType {
        ItemType::Library
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

impl Validate for LibraryDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let opts = &options.libraries;
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
            .libraries(
                ContractRules::builder()
                    .title(Req::Required)
                    .author(Req::Required)
                    .notice(Req::Required)
                    .build(),
            )
            .build()
    });

    fn parse_file(contents: &str) -> LibraryDefinition {
        let mut parser = SlangParser::builder().skip_version_detection(true).build();
        let doc = parser
            .parse_document(contents.as_bytes(), None::<std::path::PathBuf>, false)
            .unwrap();
        doc.definitions
            .into_iter()
            .find_map(Definition::to_library)
            .unwrap()
    }

    #[test]
    fn test_library() {
        let contents = "/// @title Library
        /// @author Me
        /// @notice This is a library
        library Test {}";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_library_no_natspec() {
        let contents = "library Test {}";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 3);
        assert_eq!(res.diags[0].message, "@title is missing");
        assert_eq!(res.diags[1].message, "@author is missing");
        assert_eq!(res.diags[2].message, "@notice is missing");
    }

    #[test]
    fn test_library_multiline() {
        let contents = "/**
         * @title Library
         * @author Me
         * @notice This is a library
         */
        library Test {}";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_library_inheritdoc() {
        // inheritdoc should be ignored as it doesn't apply to libraries
        let contents = "/// @inheritdoc ITest
        library Test {}";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .libraries(ContractRules::builder().title(Req::Required).build())
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@title is missing");
    }
}
