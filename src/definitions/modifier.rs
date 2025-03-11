//! Parsing and validation of modifier definitions.
use slang_solidity::cst::TextRange;

use crate::{
    lint::{check_dev, check_notice, check_params, Diagnostic, ItemDiagnostics},
    natspec::{NatSpec, NatSpecKind},
};

use super::{Attributes, Identifier, ItemType, Parent, SourceItem, Validate, ValidationOptions};

/// A modifier definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct ModifierDefinition {
    /// The parent for the modifier definition (should always be `Some`)
    pub parent: Option<Parent>,

    /// The name of the modifier
    pub name: String,

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
    /// Overridden modifiers must have inheritdoc.
    fn requires_inheritdoc(&self) -> bool {
        let parent_is_contract = matches!(self.parent, Some(Parent::Contract(_)));
        parent_is_contract && self.attributes.r#override
    }
}

impl SourceItem for ModifierDefinition {
    fn item_type() -> ItemType {
        ItemType::Modifier
    }

    fn parent(&self) -> Option<Parent> {
        self.parent.clone()
    }

    fn name(&self) -> String {
        self.name.clone()
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
            item_type: Self::item_type(),
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
        if let Some(natspec) = &self.natspec {
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
        }
        out.diags
            .extend(check_notice(&self.natspec, opts.notice, self.span()));
        out.diags
            .extend(check_dev(&self.natspec, opts.dev, self.span()));
        out.diags.extend(check_params(
            &self.natspec,
            opts.param,
            &self.params,
            self.span(),
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use semver::Version;
    use similar_asserts::assert_eq;
    use slang_solidity::{cst::NonterminalKind, parser::Parser};

    use crate::{
        config::{Enforcement, WithParamsEnforcement},
        parser::slang::Extract as _,
    };

    use super::*;

    static OPTIONS: LazyLock<ValidationOptions> =
        LazyLock::new(|| ValidationOptions::builder().inheritdoc(false).build());

    fn parse_file(contents: &str) -> ModifierDefinition {
        let parser = Parser::create(Version::new(0, 8, 0)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor
            .query(vec![ModifierDefinition::query()])
            .next()
            .unwrap();
        let def = ModifierDefinition::extract(m).unwrap();
        def.to_modifier().unwrap()
    }

    #[test]
    fn test_modifier() {
        let contents = "contract Test {
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
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");
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
            modifier foo()  { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_no_params_no_paren() {
        let contents = "contract Test {
            modifier foo { _; }
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_requires_inheritdoc() {
        let contents = "contract Test is ITest {
            modifier a() { _; }
        }";
        let res = parse_file(contents);
        assert!(!res.requires_inheritdoc());

        let contents = "contract Test is ITest {
            modifier e() override (ITest) { _; }
        }";
        let res = parse_file(contents);
        assert!(res.requires_inheritdoc());
    }

    #[test]
    fn test_modifier_inheritdoc() {
        let contents = "contract Test is ITest {
            /// @inheritdoc ITest
            modifier foo() override (ITest) { _; }
        }";
        let res = parse_file(contents).validate(&ValidationOptions::default());
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_modifier_inheritdoc_missing() {
        let contents = "contract Test is ITest {
            /// @notice Test
            modifier foo() override (ITest) { _; }
        }";
        let res = parse_file(contents).validate(&ValidationOptions::default());
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@inheritdoc is missing");
    }

    #[test]
    fn test_modifier_enforce() {
        let opts = ValidationOptions::builder()
            .inheritdoc(false)
            .modifiers(WithParamsEnforcement {
                notice: Enforcement::Required,
                dev: Enforcement::default(),
                param: Enforcement::default(),
            })
            .build();
        let contents = "contract Test {
            modifier foo() { _; }
        }";
        let res = parse_file(contents).validate(&opts);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");

        let contents = "contract Test {
            /// @notice Some notice
            modifier foo() { _; }
        }";
        let res = parse_file(contents).validate(&opts);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }
}
