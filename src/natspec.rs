//! NatSpec Comment Parser
use winnow::{
    ascii::{line_ending, space0, space1, till_line_ending},
    combinator::{alt, delimited, opt, repeat, separated},
    seq,
    token::{take_till, take_until},
    Parser as _, Result,
};

use crate::definitions::Identifier;

/// A collection of NatSpec items corresponding to a source item (function, struct, etc.)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NatSpec {
    pub items: Vec<NatSpecItem>,
}

impl NatSpec {
    /// Append the items of another [`NatSpec`] to this one's items
    pub fn append(&mut self, other: &mut Self) {
        self.items.append(&mut other.items);
    }

    /// Populate the return NatSpec items with their identifiers (which could be named `None` for unnamed returns)
    pub fn populate_returns(mut self, returns: &[Identifier]) -> Self {
        for i in self.items.iter_mut() {
            i.populate_return(returns);
        }
        self
    }

    /// Count the number of NatSpec items corresponding to a given param identifier
    pub fn count_param(&self, ident: &Identifier) -> usize {
        let Some(ident_name) = &ident.name else {
            return 0;
        };
        self.items
            .iter()
            .filter(|n| match &n.kind {
                NatSpecKind::Param { name } => name == ident_name,
                _ => false,
            })
            .count()
    }

    /// Count the number of NatSpec items corresponding to a given return identifier
    pub fn count_return(&self, ident: &Identifier) -> usize {
        let Some(ident_name) = &ident.name else {
            return 0;
        };
        self.items
            .iter()
            .filter(|n| match &n.kind {
                NatSpecKind::Return { name: Some(name) } => name == ident_name,
                _ => false,
            })
            .count()
    }

    /// Count the number of NatSpec items corresponding to an unnamed return
    pub fn count_unnamed_returns(&self) -> usize {
        self.items
            .iter()
            .filter(|n| matches!(&n.kind, NatSpecKind::Return { name: None }))
            .count()
    }

    /// Count all the return NatSpec entries for this source item
    pub fn count_all_returns(&self) -> usize {
        self.items
            .iter()
            .filter(|n| matches!(&n.kind, NatSpecKind::Return { .. }))
            .count()
    }
}

/// A single NatSpec item
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct NatSpecItem {
    /// The kind of NatSpec (notice, dev, param, etc.)
    pub kind: NatSpecKind,

    /// The comment associated with this NatSpec item
    pub comment: String,
}

impl NatSpecItem {
    /// Populate a return NatSpec item with its name if available
    ///
    /// For non-return items, this function has no effect.
    pub fn populate_return(&mut self, returns: &[Identifier]) {
        if !matches!(self.kind, NatSpecKind::Return { name: _ }) {
            return;
        }
        let name = self
            .comment
            .split_whitespace()
            .next()
            .filter(|first_word| {
                returns.iter().any(|r| match &r.name {
                    Some(name) => first_word == name,
                    None => false,
                })
            })
            .map(|first_word| first_word.to_owned());
        if let Some(name) = &name {
            if let Some(comment) = self.comment.strip_prefix(name) {
                self.comment = comment.trim_start().to_string();
            }
        }
        self.kind = NatSpecKind::Return { name }
    }

    /// Check if the item is empty (type is `@notice` - the default - and comment is empty)
    pub fn is_empty(&self) -> bool {
        self.kind == NatSpecKind::Notice && self.comment.is_empty()
    }
}

/// The kind of a NatSpec item
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NatSpecKind {
    Title,
    Author,
    Notice,
    Dev,
    Param {
        name: String,
    },
    /// For return items, [`parse_comment`] does not include the return name automatically. The [`NatSpecItem::populate_return`] must be called to retrieve the name, if any.
    Return {
        name: Option<String>,
    },
    Inheritdoc {
        parent: String,
    },
    Custom {
        tag: String,
    },
}

impl From<NatSpecItem> for NatSpec {
    fn from(value: NatSpecItem) -> Self {
        Self { items: vec![value] }
    }
}

/// Parse a Solidity doc-comment to extract the NatSpec information
pub fn parse_comment(input: &mut &str) -> Result<NatSpec> {
    alt((
        parse_single_line_comment,
        parse_multiline_comment,
        parse_empty_multiline,
    ))
    .parse_next(input)
}

fn parse_ident(input: &mut &str) -> Result<String> {
    take_till(1.., |c: char| c.is_whitespace())
        .map(|ident: &str| ident.to_owned())
        .parse_next(input)
}

fn parse_natspec_kind(input: &mut &str) -> Result<NatSpecKind> {
    alt((
        "@title".map(|_| NatSpecKind::Title),
        "@author".map(|_| NatSpecKind::Author),
        "@notice".map(|_| NatSpecKind::Notice),
        "@dev".map(|_| NatSpecKind::Dev),
        seq! {NatSpecKind::Param {
            _: "@param",
            _: space1,
            name: parse_ident
        }},
        "@return".map(|_| NatSpecKind::Return { name: None }), // we will process the name later since it's optional
        seq! {NatSpecKind::Inheritdoc {
            _: "@inheritdoc",
            _: space1,
            parent: parse_ident
        }},
        seq! {NatSpecKind::Custom {
            _: "@custom:",
            tag: parse_ident
        }},
    ))
    .parse_next(input)
}

fn parse_end_of_multiline_comment(input: &mut &str) -> Result<()> {
    let _ = (repeat::<_, _, (), (), _>(1.., '*'), '/').parse_next(input);
    Ok(())
}

fn parse_one_multiline_natspec(input: &mut &str) -> Result<NatSpecItem> {
    seq! {NatSpecItem {
        _: space0,
        _: repeat::<_, _, (), _, _>(0.., '*'),
        _: space0,
        kind: opt(parse_natspec_kind).map(|v| v.unwrap_or(NatSpecKind::Notice)),
        _: space0,
        comment: take_until(0.., ("\r", "\n", "*/")).parse_to(),
    }}
    .parse_next(input)
}

fn parse_multiline_comment(input: &mut &str) -> Result<NatSpec> {
    delimited(
        (
            '/',
            repeat::<_, _, (), _, _>(2.., '*'),
            space0,
            opt(line_ending),
        ),
        separated(0.., parse_one_multiline_natspec, line_ending),
        (opt(line_ending), space0, parse_end_of_multiline_comment),
    )
    .map(|items| NatSpec { items })
    .parse_next(input)
}

fn parse_empty_multiline(input: &mut &str) -> Result<NatSpec> {
    let _ = (
        '/',
        repeat::<_, _, (), _, _>(2.., '*'),
        space1,
        repeat::<_, _, (), _, _>(1.., '*'),
        '/',
    )
        .parse_next(input)?;
    Ok(NatSpec::default())
}

fn parse_single_line_natspec(input: &mut &str) -> Result<NatSpecItem> {
    seq! {NatSpecItem {
        _: space0,
        kind: opt(parse_natspec_kind).map(|v| v.unwrap_or(NatSpecKind::Notice)),
        _: space0,
        comment: till_line_ending.parse_to(),
    }}
    .parse_next(input)
}

fn parse_single_line_comment(input: &mut &str) -> Result<NatSpec> {
    let item = delimited(
        repeat::<_, _, (), _, _>(3.., '/'),
        parse_single_line_natspec,
        opt(line_ending),
    )
    .parse_next(input)?;
    if item.is_empty() {
        return Ok(NatSpec::default());
    }
    Ok(item.into())
}

#[cfg(test)]
mod tests {
    use similar_asserts::assert_eq;

    use super::*;

    #[test]
    fn test_kind() {
        let cases = [
            ("@title", NatSpecKind::Title),
            ("@author", NatSpecKind::Author),
            ("@notice", NatSpecKind::Notice),
            ("@dev", NatSpecKind::Dev),
            (
                "@param  foo",
                NatSpecKind::Param {
                    name: "foo".to_string(),
                },
            ),
            ("@return", NatSpecKind::Return { name: None }),
            (
                "@inheritdoc  ISomething",
                NatSpecKind::Inheritdoc {
                    parent: "ISomething".to_string(),
                },
            ),
            (
                "@custom:foo",
                NatSpecKind::Custom {
                    tag: "foo".to_string(),
                },
            ),
        ];
        for case in cases {
            let res = parse_natspec_kind.parse(case.0);
            assert!(res.is_ok(), "{res:?}");
            let res = res.unwrap();
            assert_eq!(res, case.1);
        }
    }

    #[test]
    fn test_one_multiline_item() {
        let cases = [
            ("@dev Hello world\n", NatSpecKind::Dev, "Hello world"),
            ("@title The Title\n", NatSpecKind::Title, "The Title"),
            (
                "        * @author McGyver <hi@buildanything.com>\n",
                NatSpecKind::Author,
                "McGyver <hi@buildanything.com>",
            ),
            (
                " @param foo The bar\r\n",
                NatSpecKind::Param {
                    name: "foo".to_string(),
                },
                "The bar",
            ),
            (
                " @return something The return value\n",
                NatSpecKind::Return { name: None },
                "something The return value",
            ),
            (
                "\t* @custom:foo bar\n",
                NatSpecKind::Custom {
                    tag: "foo".to_string(),
                },
                "bar",
            ),
            ("  lorem ipsum\n", NatSpecKind::Notice, "lorem ipsum"),
            ("lorem ipsum\r\n", NatSpecKind::Notice, "lorem ipsum"),
            ("\t*  foobar\n", NatSpecKind::Notice, "foobar"),
            ("    * foobar\n", NatSpecKind::Notice, "foobar"),
        ];
        for case in cases {
            let res = (parse_one_multiline_natspec, line_ending).parse(case.0);
            assert!(res.is_ok(), "{res:?}");
            let (res, _) = res.unwrap();
            assert_eq!(
                res,
                NatSpecItem {
                    kind: case.1,
                    comment: case.2.to_string()
                }
            );
        }
    }

    #[test]
    fn test_single_line() {
        let cases = [
            ("/// Foo bar\n", NatSpecKind::Notice, "Foo bar"),
            ("///  Baz\n", NatSpecKind::Notice, "Baz"),
            (
                "/// @notice  Hello world\n",
                NatSpecKind::Notice,
                "Hello world",
            ),
            (
                "/// @param foo This is bar\n",
                NatSpecKind::Param {
                    name: "foo".to_string(),
                },
                "This is bar",
            ),
            (
                "/// @return The return value\r\n",
                NatSpecKind::Return { name: None },
                "The return value",
            ),
            (
                "/// @custom:foo  This is bar\n",
                NatSpecKind::Custom {
                    tag: "foo".to_string(),
                },
                "This is bar",
            ),
        ];
        for case in cases {
            let res = parse_single_line_comment.parse(case.0);
            assert!(res.is_ok(), "{res:?}");
            let res = res.unwrap();
            assert_eq!(
                res,
                NatSpecItem {
                    kind: case.1,
                    comment: case.2.to_string()
                }
                .into()
            );
        }
    }

    #[test]
    fn test_single_line_empty() {
        let res = parse_single_line_comment.parse("///\n");
        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(res, NatSpec::default());
    }

    #[test]
    fn test_multiline() {
        let comment = "/**
     * @notice Some notice text.
     */";
        let res = parse_multiline_comment.parse(comment);
        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(
            res,
            NatSpec {
                items: vec![NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Some notice text.".to_string()
                }]
            }
        );
    }

    #[test]
    fn test_multiline2() {
        let comment = "/**
     * @notice Some notice text.
     * @custom:something
     */";
        let res = parse_multiline_comment.parse(comment);
        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(
            res,
            NatSpec {
                items: vec![
                    NatSpecItem {
                        kind: NatSpecKind::Notice,
                        comment: "Some notice text.".to_string()
                    },
                    NatSpecItem {
                        kind: NatSpecKind::Custom {
                            tag: "something".to_string()
                        },
                        comment: String::new()
                    }
                ]
            }
        );
    }

    #[test]
    fn test_multiline3() {
        let comment = "/** @notice Some notice text.
Another notice
        * @param test
     \t** @custom:something */";
        let res = parse_multiline_comment.parse(comment);
        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(
            res,
            NatSpec {
                items: vec![
                    NatSpecItem {
                        kind: NatSpecKind::Notice,
                        comment: "Some notice text.".to_string()
                    },
                    NatSpecItem {
                        kind: NatSpecKind::Notice,
                        comment: "Another notice".to_string()
                    },
                    NatSpecItem {
                        kind: NatSpecKind::Param {
                            name: "test".to_string()
                        },
                        comment: "".to_string()
                    },
                    NatSpecItem {
                        kind: NatSpecKind::Custom {
                            tag: "something".to_string()
                        },
                        comment: String::new()
                    }
                ]
            }
        );
    }

    #[test]
    fn test_multiline_empty() {
        let comment = "/**
        */";
        let res = parse_comment.parse(comment);
        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(res, NatSpec::default());

        let comment = "/** */";
        let res = parse_comment.parse(comment);
        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(res, NatSpec::default());
    }

    #[test]
    fn test_multiline_weird() {
        let comment = "/**** @notice Some text
    ** */";
        let res = parse_comment.parse(comment);
        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(
            res,
            NatSpec {
                items: vec![
                    NatSpecItem {
                        kind: NatSpecKind::Notice,
                        comment: "Some text".to_string()
                    },
                    NatSpecItem {
                        kind: NatSpecKind::Notice,
                        comment: "".to_string()
                    }
                ]
            }
        );
    }
}
