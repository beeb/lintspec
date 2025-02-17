//! NatSpec Comment Parser
use winnow::{
    ascii::{line_ending, multispace0, space0, space1},
    combinator::{alt, delimited, opt, preceded, repeat, separated, terminated},
    seq,
    stream::AsChar,
    token::take_till,
    Parser as _, Result,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NatSpec {
    pub items: Vec<NatSpecItem>,
}

impl NatSpec {
    pub fn append(&mut self, other: &mut Self) {
        self.items.append(&mut other.items);
    }

    pub fn populate_returns<'a>(mut self, returns: impl Iterator<Item = &'a str> + Clone) -> Self {
        for i in self.items.iter_mut() {
            i.populate_return(returns.clone());
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatSpecItem {
    pub kind: NatSpecKind,
    pub comment: String,
}

impl NatSpecItem {
    pub fn populate_return<'a>(&mut self, mut returns: impl Iterator<Item = &'a str>) {
        if !matches!(self.kind, NatSpecKind::Return { name: _ }) {
            return;
        }
        self.kind = NatSpecKind::Return {
            name: self
                .comment
                .split_whitespace()
                .next()
                .filter(|first_word| returns.any(|r| r == *first_word))
                .map(|first_word| first_word.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NatSpecKind {
    Title,
    Author,
    Notice,
    Dev,
    Param { name: String },
    Return { name: Option<String> },
    Inheritdoc { parent: String },
    Custom { tag: String },
}

impl From<NatSpecItem> for NatSpec {
    fn from(value: NatSpecItem) -> Self {
        Self { items: vec![value] }
    }
}

pub fn parse_comment(input: &mut &str) -> Result<NatSpec> {
    alt((
        parse_multiline_comment,
        parse_single_line_comment.map(|c| c.into()),
    ))
    .parse_next(input)
}

fn parse_ident(input: &mut &str) -> Result<String> {
    take_till(1.., |c: char| c.is_whitespace())
        .map(|parent: &str| parent.to_owned())
        .parse_next(input)
}

fn parse_natspec_kind(input: &mut &str) -> Result<NatSpecKind> {
    terminated(
        alt((
            "@title".map(|_| NatSpecKind::Title),
            "@author".map(|_| NatSpecKind::Author),
            "@notice".map(|_| NatSpecKind::Notice),
            "@dev".map(|_| NatSpecKind::Dev),
            seq!(NatSpecKind::Param {
                _: "@param",
                _: space1,
                name: parse_ident
            }),
            "@return".map(|_| NatSpecKind::Return { name: None }), // we will process the name later since it's optional
            seq!(NatSpecKind::Inheritdoc {
                _: "@inheritdoc",
                _: space1,
                parent: parse_ident
            }),
            seq!(NatSpecKind::Custom {
                _: "@custom:",
                tag: parse_ident
            }),
        )),
        space0,
    )
    .parse_next(input)
}

fn parse_comment_line(input: &mut &str) -> Result<NatSpecItem> {
    seq! {NatSpecItem {
        _: opt(delimited(multispace0, '*', space0)),
        kind: opt(parse_natspec_kind).map(|v| v.unwrap_or(NatSpecKind::Notice)),
        comment: take_till(0.., |c: char| c.is_newline()).map(|s: &str| s.to_owned())
    }}
    .parse_next(input)
}

fn parse_multiline_comment(input: &mut &str) -> Result<NatSpec> {
    delimited("/**", separated(1.., parse_comment_line, line_ending), "*/")
        .map(|items| NatSpec { items })
        .parse_next(input)
}

fn parse_single_line_comment(input: &mut &str) -> Result<NatSpecItem> {
    preceded(("///", space0), parse_comment_line).parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kind() {
        let cases = [
            ("@title", NatSpecKind::Title),
            ("@title ", NatSpecKind::Title),
            ("@title  ", NatSpecKind::Title),
            ("@author", NatSpecKind::Author),
            ("@notice", NatSpecKind::Notice),
            ("@dev", NatSpecKind::Dev),
            (
                "@param  foo  ",
                NatSpecKind::Param {
                    name: "foo".to_string(),
                },
            ),
            ("@return", NatSpecKind::Return { name: None }),
            (
                "@inheritdoc  ISomething  ",
                NatSpecKind::Inheritdoc {
                    parent: "ISomething".to_string(),
                },
            ),
            (
                "@custom:foo  ",
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
    fn test_single_line() {
        let cases = [
            ("/// Foo bar", NatSpecKind::Notice, "Foo bar"),
            ("///  Baz", NatSpecKind::Notice, "Baz"),
            (
                "/// @notice  Hello world",
                NatSpecKind::Notice,
                "Hello world",
            ),
            (
                "/// @param foo This is bar",
                NatSpecKind::Param {
                    name: "foo".to_string(),
                },
                "This is bar",
            ),
            (
                "/// @return The return value",
                NatSpecKind::Return { name: None },
                "The return value",
            ),
            (
                "/// @custom:foo  This is bar",
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
            );
        }
    }
}
