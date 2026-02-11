use std::sync::LazyLock;

pub static INTERNER: LazyLock<Interner> = LazyLock::new(Interner::new);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Symbol(inturn::Symbol);

impl Symbol {
    pub fn resolve_with(self, interner: &'static Interner) -> &'static str {
        interner.resolve(self)
    }
}

#[derive(Default)]
pub struct Interner(inturn::Interner);

impl Interner {
    #[must_use]
    pub fn new() -> Self {
        Self(inturn::Interner::new())
    }

    pub fn get_or_intern(&self, string: impl AsRef<str>) -> Symbol {
        Symbol(self.0.intern(string.as_ref()))
    }

    pub fn get_or_intern_static(&self, string: &'static str) -> Symbol {
        Symbol(self.0.intern_static(string))
    }

    pub fn resolve(&'static self, sym: Symbol) -> &'static str {
        self.0.resolve(sym.0)
    }
}
