//! Parsing and validation of event definitions.
use slang_solidity::cst::TextRange;

use crate::{
    lint::{check_dev, check_notice, check_params, ItemDiagnostics},
    natspec::NatSpec,
};

use super::{Identifier, ItemType, Parent, SourceItem, Validate, ValidationOptions};

/// An event definition
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct EventDefinition {
    /// The parent for the event definition, if any
    pub parent: Option<Parent>,

    /// The name of the event
    pub name: String,

    /// The span of the event definition
    pub span: TextRange,

    /// The name and span of the event's parameters
    pub params: Vec<Identifier>,

    /// The [`NatSpec`] associated with the event definition, if any
    pub natspec: Option<NatSpec>,
}

impl SourceItem for EventDefinition {
    fn item_type() -> ItemType {
        ItemType::Event
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

impl Validate for EventDefinition {
    fn validate(&self, options: &ValidationOptions) -> ItemDiagnostics {
        let opts = &options.events;
        println!("{opts:?}");
        let mut out = ItemDiagnostics {
            parent: self.parent(),
            item_type: Self::item_type(),
            name: self.name(),
            span: self.span(),
            diags: vec![],
        };
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
        config::{Req, WithParamsRules},
        parser::slang::Extract as _,
    };

    use super::*;

    static OPTIONS: LazyLock<ValidationOptions> =
        LazyLock::new(|| ValidationOptions::builder().inheritdoc(false).build());

    fn parse_file(contents: &str) -> EventDefinition {
        let parser = Parser::create(Version::new(0, 8, 26)).unwrap();
        let output = parser.parse(NonterminalKind::SourceUnit, contents);
        assert!(output.is_valid(), "{:?}", output.errors());
        let cursor = output.create_tree_cursor();
        let m = cursor.query(vec![EventDefinition::query()]).next().unwrap();
        let def = EventDefinition::extract(m).unwrap();
        def.to_event().unwrap()
    }

    #[test]
    fn test_event() {
        let contents = "contract Test {
            /// @param a The first
            /// @param b The second
            event Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_event_no_natspec() {
        let contents = "contract Test {
            event Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param a is missing");
        assert_eq!(res.diags[1].message, "@param b is missing");
    }

    #[test]
    fn test_event_only_notice() {
        let contents = "contract Test {
            /// @notice
            event Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param a is missing");
        assert_eq!(res.diags[1].message, "@param b is missing");
    }

    #[test]
    fn test_event_one_missing() {
        let contents = "contract Test {
            /// @param a The first
            event Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param b is missing");
    }

    #[test]
    fn test_event_multiline() {
        let contents = "contract Test {
            /**
             * @param a The first
             * @param b The second
             */
            event Foobar(uint256 a, uint256 b);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_event_duplicate() {
        let contents = "contract Test {
            /// @param a The first
            /// @param a The first again
            event Foobar(uint256 a);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param a is present more than once");
    }

    #[test]
    fn test_event_no_params() {
        let contents = "contract Test {
            event Foobar();
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_event_inheritdoc() {
        let contents = "contract Test {
            /// @inheritdoc ITest
            event Foobar(uint256 a);
        }";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "@param a is missing");
    }

    #[test]
    fn test_event_enforce() {
        let opts = ValidationOptions::builder()
            .inheritdoc(false)
            .events(WithParamsRules {
                notice: Req::Required,
                dev: Req::default(),
                param: Req::default(),
            })
            .build();
        let contents = "contract Test {
            event Foobar();
        }";
        let res = parse_file(contents).validate(&opts);
        assert_eq!(res.diags.len(), 1);
        assert_eq!(res.diags[0].message, "missing NatSpec");

        let contents = "contract Test {
            /// @notice Some notice
            event Foobar();
        }";
        let res = parse_file(contents).validate(&opts);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_event_no_contract() {
        let contents = "
            /// @param a The first
            /// @param b The second
            event Foobar(uint256 a, uint256 b);
            ";
        let res = parse_file(contents).validate(&OPTIONS);
        assert!(res.diags.is_empty(), "{:#?}", res.diags);
    }

    #[test]
    fn test_event_no_contract_missing() {
        let contents = "event Foobar(uint256 a, uint256 b);";
        let res = parse_file(contents).validate(&OPTIONS);
        assert_eq!(res.diags.len(), 2);
        assert_eq!(res.diags[0].message, "@param a is missing");
        assert_eq!(res.diags[1].message, "@param b is missing");
    }
}
