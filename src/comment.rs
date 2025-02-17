//! NatSpec Comment Parser
use winnow::{
    ascii::{space0, space1},
    combinator::{alt, delimited, opt, preceded, repeat},
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatSpecItem {
    pub kind: NatSpecKind,
    pub comment: String,
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
    ))
    .parse_next(input)
}

fn parse_comment_line(input: &mut &str) -> Result<NatSpecItem> {
    seq! {NatSpecItem {
        _: opt(delimited(space0, '*', space0)),
        kind: opt(parse_natspec_kind).map(|v| v.unwrap_or(NatSpecKind::Notice)),
        comment: take_till(0.., |c: char| c.is_newline()).map(|s: &str| s.to_owned())
    }}
    .parse_next(input)
}

fn parse_multiline_comment(input: &mut &str) -> Result<NatSpec> {
    delimited("/**", repeat(0.., parse_comment_line), "*/")
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
    fn test_single_line() {
        let res = parse_single_line_comment.parse("/// Foo bar");
        assert!(res.is_ok(), "{res:?}");
        let res = res.unwrap();
        assert_eq!(
            res,
            NatSpecItem {
                kind: NatSpecKind::Notice,
                comment: "Foo bar".to_string()
            }
        );
    }
}
