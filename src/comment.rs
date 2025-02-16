//! NatSpec Comment Parser

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
