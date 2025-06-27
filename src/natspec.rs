//! `NatSpec` Comment Parser
use std::ops::Range;

use derive_more::IsVariant;
use winnow::{
    ascii::{line_ending, space0, space1, till_line_ending},
    combinator::{alt, cut_err, delimited, not, opt, repeat, separated},
    error::{StrContext, StrContextValue},
    seq,
    token::{take_till, take_until},
    LocatingSlice, ModalResult, Parser,
};

use crate::definitions::{Identifier, TextIndex, TextRange};

/// A collection of `NatSpec` items corresponding to a source item (function, struct, etc.)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NatSpec {
    pub items: Vec<NatSpecItem>,
}

impl NatSpec {
    /// Append the items of another [`NatSpec`] to this one's items
    pub fn append(&mut self, other: &mut Self) {
        self.items.append(&mut other.items);
    }

    /// Populate the return `NatSpec` items with their identifiers (which could be named `None` for unnamed returns)
    #[must_use]
    pub fn populate_returns(mut self, returns: &[Identifier]) -> Self {
        for i in &mut self.items {
            i.populate_return(returns);
        }
        self
    }

    /// Count the number of `NatSpec` items corresponding to a given param identifier
    #[must_use]
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

    /// Count the number of `NatSpec` items corresponding to a given return identifier
    #[must_use]
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

    /// Count the number of `NatSpec` items corresponding to an unnamed return
    #[must_use]
    pub fn count_unnamed_returns(&self) -> usize {
        self.items
            .iter()
            .filter(|n| matches!(&n.kind, NatSpecKind::Return { name: None }))
            .count()
    }

    /// Count all the return `NatSpec` entries for this source item
    #[must_use]
    pub fn count_all_returns(&self) -> usize {
        self.items.iter().filter(|n| n.kind.is_return()).count()
    }

    #[must_use]
    pub fn has_param(&self) -> bool {
        self.items.iter().any(|n| n.kind.is_param())
    }

    #[must_use]
    pub fn has_return(&self) -> bool {
        self.items.iter().any(|n| n.kind.is_return())
    }

    #[must_use]
    pub fn has_notice(&self) -> bool {
        self.items.iter().any(|n| n.kind.is_notice())
    }

    #[must_use]
    pub fn has_dev(&self) -> bool {
        self.items.iter().any(|n| n.kind.is_dev())
    }
}

/// A single `NatSpec` item
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
#[builder(on(String, into))]
pub struct NatSpecItem {
    /// The kind of `NatSpec` (notice, dev, param, etc.)
    pub kind: NatSpecKind,

    /// The comment associated with this `NatSpec` item
    pub comment: String,

    /// The span of this item, relative to the start offset of the string passed to [`parse_comment`]
    pub span: TextRange,
}

impl NatSpecItem {
    /// Populate a return `NatSpec` item with its name if available
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
            .map(ToOwned::to_owned);
        if let Some(name) = &name {
            if let Some(comment) = self.comment.strip_prefix(name) {
                self.comment = comment.trim_start().to_string();
            }
        }
        self.kind = NatSpecKind::Return { name }
    }

    /// Check if the item is empty (type is `@notice` - the default - and comment is empty)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kind == NatSpecKind::Notice && self.comment.is_empty()
    }
}

/// The kind of a `NatSpec` item
#[derive(Debug, Clone, PartialEq, Eq, IsVariant)]
pub enum NatSpecKind {
    Title,
    Author,
    Notice,
    Dev,
    Param {
        name: String,
    },
    /// For return items, [`parse_comment`] does not include the return name automatically. The [`NatSpecItem::populate_return`] function must be called to retrieve the name, if any.
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

/// Parse a Solidity doc-comment to extract the `NatSpec` information
pub fn parse_comment(input: &mut &str) -> ModalResult<NatSpec> {
    let (mut natspec, spans) = alt((single_line_comment, multiline_comment, empty_multiline))
        .parse_next(&mut LocatingSlice::new(input))?;
    if natspec.items.is_empty() {
        return Ok(natspec);
    }

    let mut current_index = TextIndex::ZERO;
    let mut char_iter = input.chars().peekable();
    for (natspec_item, byte_span) in natspec.items.iter_mut().zip(spans.iter()) {
        if current_index.utf8 == byte_span.start {
            natspec_item.span.start = current_index;
        } else {
            // find start offset
            while let Some(c) = char_iter.next() {
                current_index.advance(c, char_iter.peek());
                if current_index.utf8 == byte_span.start {
                    natspec_item.span.start = current_index;
                    break;
                }
            }
        }
        // find end offset
        while let Some(c) = char_iter.next() {
            current_index.advance(c, char_iter.peek());
            if current_index.utf8 == byte_span.end {
                natspec_item.span.end = current_index;
                break;
            }
        }
    }
    Ok(natspec)
}

/// Parse an identifier (contiguous non-whitespace characters)
fn ident(input: &mut LocatingSlice<&str>) -> ModalResult<String> {
    take_till(1.., |c: char| c.is_whitespace())
        .map(|ident: &str| ident.to_owned())
        .parse_next(input)
}

/// Parse a [`NatSpecKind`] (tag followed by an optional identifier)"
///
/// For `@return`, the identifier, if present, is not included in the `NatSpecItem` for now. A post-processing
/// step ([`NatSpecItem::populate_return`]) is needed to extract the name.
fn natspec_kind(input: &mut LocatingSlice<&str>) -> ModalResult<NatSpecKind> {
    alt((
        "@title".map(|_| NatSpecKind::Title),
        "@author".map(|_| NatSpecKind::Author),
        "@notice".map(|_| NatSpecKind::Notice),
        "@dev".map(|_| NatSpecKind::Dev),
        seq! {NatSpecKind::Param {
            _: "@param",
            _: space1,
            name: ident
        }},
        "@return".map(|_| NatSpecKind::Return { name: None }), // we will process the name later since it's optional
        seq! {NatSpecKind::Inheritdoc {
            _: "@inheritdoc",
            _: space1,
            parent: ident
        }},
        seq! {NatSpecKind::Custom {
            _: "@custom:",
            tag: ident
        }},
    ))
    .parse_next(input)
}

/// Parse the end of a multiline comment (one or more `*` followed by `/`)
#[allow(clippy::unnecessary_wraps)]
fn end_of_comment(input: &mut LocatingSlice<&str>) -> ModalResult<()> {
    let _ = (repeat::<_, _, (), (), _>(1.., '*'), '/').parse_next(input);
    Ok(())
}

/// Parse a single `NatSpec` item (line) in a multiline comment
fn one_multiline_natspec(
    input: &mut LocatingSlice<&str>,
) -> ModalResult<(NatSpecItem, Range<usize>)> {
    let _ = space0.parse_next(input)?;
    let () = repeat::<_, _, (), _, _>(0.., '*').parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let (kind, kind_span) = opt(natspec_kind)
        .map(|v| v.unwrap_or(NatSpecKind::Notice))
        .with_span()
        .parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let (comment, comment_span) = take_until(0.., ("\r", "\n", "*/"))
        .parse_to()
        .with_span()
        .parse_next(input)?;
    Ok((
        NatSpecItem {
            kind,
            comment,
            span: TextRange::default(),
        },
        kind_span.start..comment_span.end,
    ))
}

/// Parse a multiline `NatSpec` comment
fn multiline_comment(input: &mut LocatingSlice<&str>) -> ModalResult<(NatSpec, Vec<Range<usize>>)> {
    delimited(
        (
            (
                "/**",
                // three stars is not a valid doc-comment
                // <https://github.com/ethereum/solidity/issues/9139>
                cut_err(not('*'))
                    .context(StrContext::Label("delimiter"))
                    .context(StrContext::Expected(StrContextValue::Description("/**"))),
            ),
            space0,
            opt(line_ending),
        ),
        separated(0.., one_multiline_natspec, line_ending),
        (opt(line_ending), space0, end_of_comment),
    )
    .map(|items: Vec<(NatSpecItem, Range<usize>)>| {
        let (items, spans) = items.into_iter().unzip();
        (NatSpec { items }, spans)
    })
    .parse_next(input)
}

/// Parse an empty multiline comment (without any text in the body)
fn empty_multiline(input: &mut LocatingSlice<&str>) -> ModalResult<(NatSpec, Vec<Range<usize>>)> {
    let _ = ("/**", space1, repeat::<_, _, (), _, _>(1.., '*'), '/').parse_next(input)?;
    Ok((NatSpec::default(), Vec::new()))
}

/// Parse a single line comment `NatSpec` item
fn single_line_natspec(
    input: &mut LocatingSlice<&str>,
) -> ModalResult<(NatSpecItem, Range<usize>)> {
    let _ = space0.parse_next(input)?;
    let (kind, kind_span) = opt(natspec_kind)
        .map(|v| v.unwrap_or(NatSpecKind::Notice))
        .with_span()
        .parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let (comment, comment_span) = till_line_ending.parse_to().with_span().parse_next(input)?;
    Ok((
        NatSpecItem {
            kind,
            comment,
            span: TextRange::default(),
        },
        kind_span.start..comment_span.end,
    ))
}

/// Parse a single line `NatSpec` comment
fn single_line_comment(
    input: &mut LocatingSlice<&str>,
) -> ModalResult<(NatSpec, Vec<Range<usize>>)> {
    let (item, range) = delimited(
        (
            "///",
            // four slashes is not a valid doc-comment
            // <https://github.com/ethereum/solidity/issues/9139>
            cut_err(not('/'))
                .context(StrContext::Label("delimiter"))
                .context(StrContext::Expected(StrContextValue::Description("///"))),
        ),
        single_line_natspec,
        opt(line_ending),
    )
    .parse_next(input)?;
    if item.is_empty() {
        return Ok((NatSpec::default(), Vec::new()));
    }
    Ok((item.into(), vec![range]))
}

#[cfg(test)]
mod tests {
    use similar_asserts::assert_eq;
    use winnow::error::ParseError;

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
            let res = natspec_kind.parse(LocatingSlice::new(case.0));
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
            let res = (one_multiline_natspec, line_ending).parse(LocatingSlice::new(case.0));
            assert!(res.is_ok(), "{res:?}");
            let ((res, _), _) = res.unwrap();
            assert_eq!(
                res,
                NatSpecItem {
                    kind: case.1,
                    comment: case.2.to_string(),
                    span: TextRange::default()
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
            let res = single_line_comment.parse(LocatingSlice::new(case.0));
            assert!(res.is_ok(), "{res:?}");
            let (res, _) = res.unwrap();
            assert_eq!(
                res,
                NatSpecItem {
                    kind: case.1,
                    comment: case.2.to_string(),
                    span: TextRange::default()
                }
                .into()
            );
        }
    }

    #[test]
    fn test_single_line_empty() {
        let res = single_line_comment.parse(LocatingSlice::new("///\n"));
        assert!(res.is_ok(), "{res:?}");
        let (res, _) = res.unwrap();
        assert_eq!(res, NatSpec::default());
    }

    #[test]
    fn test_single_line_weird() {
        let res = single_line_comment.parse(LocatingSlice::new("//// Hello\n"));
        assert!(matches!(res, Err(ParseError { .. })));
    }

    #[test]
    fn test_multiline() {
        let comment = "/**
     * @notice Some notice text.
     */";
        let res = multiline_comment.parse(LocatingSlice::new(comment));
        assert!(res.is_ok(), "{res:?}");
        let (res, _) = res.unwrap();
        assert_eq!(
            res,
            NatSpec {
                items: vec![NatSpecItem {
                    kind: NatSpecKind::Notice,
                    comment: "Some notice text.".to_string(),
                    span: TextRange::default()
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
        let res = multiline_comment.parse(LocatingSlice::new(comment));
        assert!(res.is_ok(), "{res:?}");
        let (res, _) = res.unwrap();
        assert_eq!(
            res,
            NatSpec {
                items: vec![
                    NatSpecItem {
                        kind: NatSpecKind::Notice,
                        comment: "Some notice text.".to_string(),
                        span: TextRange::default()
                    },
                    NatSpecItem {
                        kind: NatSpecKind::Custom {
                            tag: "something".to_string()
                        },
                        comment: String::new(),
                        span: TextRange::default()
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
        let res = multiline_comment.parse(LocatingSlice::new(comment));
        assert!(res.is_ok(), "{res:?}");
        let (res, _) = res.unwrap();
        assert_eq!(
            res,
            NatSpec {
                items: vec![
                    NatSpecItem {
                        kind: NatSpecKind::Notice,
                        comment: "Some notice text.".to_string(),
                        span: TextRange::default()
                    },
                    NatSpecItem {
                        kind: NatSpecKind::Notice,
                        comment: "Another notice".to_string(),
                        span: TextRange::default()
                    },
                    NatSpecItem {
                        kind: NatSpecKind::Param {
                            name: "test".to_string()
                        },
                        comment: String::new(),
                        span: TextRange::default()
                    },
                    NatSpecItem {
                        kind: NatSpecKind::Custom {
                            tag: "something".to_string()
                        },
                        comment: String::new(),
                        span: TextRange::default()
                    }
                ]
            }
        );
    }

    #[test]
    fn test_multiline_empty() {
        let mut comment = "/**
        */";
        let res = parse_comment(&mut comment); // this is fine

        // let comment = "/**
        // */";
        // let res = parse_comment.parse(comment); // this errors

        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(res, NatSpec::default());

        let mut comment = "/** */";
        let res = parse_comment(&mut comment); // this is fine

        // let comment = "/** */";
        // let res = parse_comment.parse(comment); // this errors

        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(res, NatSpec::default());
    }

    #[test]
    fn test_multiline_weird() {
        let comment = "/**** @notice Some text
    ** */";
        let res = parse_comment.parse(comment);
        assert!(matches!(res, Err(ParseError { .. })));
    }
}
