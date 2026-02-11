use std::sync::LazyLock;

use lasso::{Spur, ThreadedRodeo};

pub static INTERNER: LazyLock<Interner> = LazyLock::new(Interner::new);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Symbol(Spur);

impl Symbol {
    pub fn resolve_with(self, interner: &'static Interner) -> &'static str {
        interner.resolve(self)
    }
}

#[derive(Debug, Default)]
pub struct Interner(ThreadedRodeo);

impl Interner {
    #[must_use]
    pub fn new() -> Self {
        Self(ThreadedRodeo::new())
    }

    pub fn get_or_intern(&self, string: impl AsRef<str>) -> Symbol {
        Symbol(self.0.get_or_intern(string))
    }

    pub fn get_or_intern_static(&self, string: &'static str) -> Symbol {
        Symbol(self.0.get_or_intern_static(string))
    }

    pub fn resolve(&'static self, sym: Symbol) -> &'static str {
        self.0.resolve(&sym.0)
    }
}
