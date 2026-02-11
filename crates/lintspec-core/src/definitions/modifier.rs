//! Parsing and validation of modifier definitions.
use crate::{
    interner::{INTERNER, Symbol},
    lint::{CheckNoticeAndDev, CheckParams, Diagnostic, ItemDiagnostics},
    natspec::{NatSpec, NatSpecKind},
};

use super::{
    Attributes, Identifier, ItemType, Parent, SourceItem, TextRange, Validate, ValidationOptions,
};

/// A modifier definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct ModifierDefinition {
    /// The parent for the modifier definition (should always be `Some`)
    pub parent: Option<Parent>,

    /// The name of the modifier
    pub name: Symbol,

    /// The span of the modifier definition, exluding the body
    pub span: TextRange,

    /// The name and span of the modifier's parameters
    pub params: Vec<Identifier>,

    /// The [`NatSpec`] associated with the modifier definition, if any
    pub natspec: Option<NatSpec>,

    /// The attributes of the modifier (override)
    pub attributes: Attributes,
}

impl ModifierDefinition {
    /// Check whether this modifier requires inheritdoc when we enforce it
    ///
    /// `override` modifiers must have inheritdoc.
    fn requires_inheritdoc(&self, options: &ValidationOptions) -> bool {
        let parent_is_contract = matches!(self.parent, Some(Parent::Contract(_)));
        options.inheritdoc_override && self.attributes.r#override && parent_is_contract
    }
}

impl SourceItem for ModifierDefinition {
    fn item_type(&self) -> ItemType {
        ItemType::Modifier
    }

    fn parent(&self) -> Option<Parent> {
        self.parent.clone()
    }

    fn name(&self) -> Symbol {
        self.name
    }

    fn span(&self) -> TextRange {
        self.span.clone()
    }
}

impl Validate for ModifierDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let opts = &options.modifiers;
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: self.item_type(),
            name: self.name().resolve_with(&INTERNER),
            span: self.span(),
            diags: vec![],
        };
        if let Some(natspec) = &self.natspec
            && natspec
                .items
                .iter()
                .any(|n| matches!(n.kind, NatSpecKind::Inheritdoc { .. }))
        {
            // if there is `inheritdoc`, no further validation is required
            return out;
        } else if self.requires_inheritdoc(options) {
            out.diags.push(Diagnostic {
                span: self.span(),
                message: "@inheritdoc is missing".to_string(),
            });
            return out;
        }
        out.diags.extend(
            CheckNoticeAndDev::builder()
                .natspec(&self.natspec)
                .notice_rule(opts.notice)
                .dev_rule(opts.dev)
                .notice_or_dev(options.notice_or_dev)
                .span(&self.span)
                .build()
                .check(),
        );
        out.diags.extend(
            CheckParams::builder()
                .natspec(&self.natspec)
                .rule(opts.param)
                .params(&self.params)
                .default_span(self.span())
                .build()
                .check(),
        );
        out
    }
}

#[cfg(test)]
#[cfg(feature = "solar")]
mod tests {
    use std::sync::LazyLock;

    use similar_asserts::assert_eq;

    use crate::{
        definitions::Definition,
        parser::{Parse as _, solar::SolarParser},
    };

    use super::*;

    static OPTIONS: LazyLock<ValidationOptions> =
        LazyLock::new(|| ValidationOptions::builder().inheritdoc(false).build());

    fn parse_file(contents: &str) -> ModifierDefinition {
        let mut parser = SolarParser::default();
        let doc = parser
            .parse_document(contents.as_bytes(), None::<std::path::PathBuf>, false)
            .unwrap();
        doc.definitions
            .into_iter()
            .find_map(Definition::to_modifier)
            .unwrap()
    }

    #[test]
    fn test_modifier() {
        let contents = "contract Test {
            /// @notice A modifier
            /// @param param1 Test
            /// @param param2 Test2
            modifier foo(uint256 param1, bytes calldata param2) { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_no_natspec() {
        let contents = "contract Test {
            modifier foo(uint256 param1, bytes calldata param2) { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 3);
        assert_eq!(res.diags[0].message, "@notice is missing");
        assert_eq!(res.diags[1].message, "@param param1 is missing");
        assert_eq!(res.diags[2].message, "@param param2 is missing");
    }

    #[test]
    fn test_modifier_only_notice() {
        let contents = "contract Test {
            /// @notice The modifier
            modifier foo(uint256 param1, bytes calldata param2) { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param param1 is missing");
        assert_eq!(res.diags[1].message, "@param param2 is missing");
    }

    #[test]
    fn test_modifier_one_missing() {
        let contents = "contract Test {
            /// @notice A modifier
            /// @param param1 The first
            modifier foo(uint256 param1, bytes calldata param2) { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param param2 is missing");
    }

    #[test]
    fn test_modifier_multiline() {
        let contents = "contract Test {
            /**
             * @notice A modifier
             * @param param1 Test
             * @param param2 Test2
             */
            modifier foo(uint256 param1, bytes calldata param2) { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_duplicate() {
        let contents = "contract Test {
            /// @notice A modifier
            /// @param param1 The first
            /// @param param1 The first again
            modifier foo(uint256 param1) { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(
            res.diags[0].message,
            "@param param1 is present more than once"
        );
    }

    #[test]
    fn test_modifier_no_params() {
        let contents = "contract Test {
            /// @notice A modifier
            modifier foo()  { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_no_params_no_paren() {
        let contents = "contract Test {
            /// @notice A modifier
            modifier foo { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_requires_inheritdoc() {
        let options = ValidationOptions::builder()
            .inheritdoc_override(true)
            .build();

        let contents = "contract Test is ITest {
            modifier a() { _; }
        }";
        let res = parse_file(contents);
        assert!(!res.requires_inheritdoc(&options));
        assert!(!res.requires_inheritdoc(&ValidationOptions::default()));

        let contents = "contract Test is ITest {
            modifier e() override (ITest) { _; }
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc(&options));
        assert!(!res.requires_inheritdoc(&ValidationOptions::default()));
    }

    #[test]
    fn test_modifier_inheritdoc() {
        let contents = "contract Test is ITest {
            /// @inheritdoc ITest
            modifier foo() override (ITest) { _; }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc_override(true)
                .build(),
        );
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_inheritdoc_missing() {
        let contents = "contract Test is ITest {
            /// @notice Test
            modifier foo() override (ITest) { _; }
        }";
        let res = parse_file(contents).validate(
            &ValidationOptions::builder()
                .inheritdoc_override(true)
                .build(),
        );
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@inheritdoc is missing");
    }
}
