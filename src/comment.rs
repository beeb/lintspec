//! NatSpec Comment Parser

use chumsky::{
    error::Simple,
    prelude::{choice, just, take_until},
    text::{self, TextParser as _},
    Parser,
};

#[derive(Debug, Clone, Default)]
pub struct NatSpec {
    pub items: Vec<NatSpecItem>,
}

#[derive(Debug, Clone)]
pub struct NatSpecItem {
    pub kind: NatSpecKind,
    pub comment: String,
}

#[derive(Debug, Clone)]
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

pub fn parser() -> impl Parser<char, NatSpec, Error = Simple<char>> {
    choice((multi_line(), single_line().map(NatSpec::from)))
}

fn natspec_kind() -> impl Parser<char, NatSpecKind, Error = Simple<char>> {
    choice((
        just("@title").to(NatSpecKind::Title),
        just("@author").to(NatSpecKind::Author),
        just("@notice").to(NatSpecKind::Notice),
        just("@dev").to(NatSpecKind::Dev),
        just("@param")
            .padded()
            .then(text::ident())
            .map(|(name, _)| NatSpecKind::Param {
                name: name.to_string(),
            }),
        just("@return").to(NatSpecKind::Return { name: None }), // we will process the name later since it's optional
        just("@inheritdoc")
            .padded()
            .then(text::ident())
            .map(|(parent, _)| NatSpecKind::Inheritdoc {
                parent: parent.to_string(),
            }),
        just("@custom:")
            .then(text::ident())
            .map(|(tag, _)| NatSpecKind::Custom {
                tag: tag.to_string(),
            }),
    ))
}

fn comment_line() -> impl Parser<char, NatSpecItem, Error = Simple<char>> {
    just('*')
        .or_not()
        .padded()
        .ignore_then(natspec_kind().or_not())
        .then(take_until(text::newline()))
        .map(|(kind, (comment, _))| NatSpecItem {
            kind: kind.unwrap_or(NatSpecKind::Notice),
            comment: comment.into_iter().collect(),
        })
}

fn multi_line() -> impl Parser<char, NatSpec, Error = Simple<char>> {
    comment_line()
        .repeated()
        .delimited_by(just("/**"), just("*/"))
        .map(|list| NatSpec { items: list })
}

fn single_line() -> impl Parser<char, NatSpecItem, Error = Simple<char>> {
    just("///").padded().ignore_then(comment_line())
}
