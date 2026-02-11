//! String interner used by data structures.
use std::sync::LazyLock;

/// The global string interner used by the parsers and other data structures.
pub static INTERNER: LazyLock<Interner> = LazyLock::new(Interner::new);

/// The reference to an interned string from the [`INTERNER`].
///
/// This type can be compared to rapidly check for string equality.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Symbol(inturn::Symbol);

impl Symbol {
    /// Resolve the symbol to its interned string slice.
    pub fn resolve_with(self, interner: &'static Interner) -> &'static str {
        interner.resolve(self)
    }
}

/// A string interner, uses [`inturn`] under the hood.
#[derive(Default)]
pub struct Interner(inturn::Interner);

impl Interner {
    /// Create a new interner.
    #[must_use]
    pub fn new() -> Self {
        Self(inturn::Interner::new())
    }

    /// Intern a string reference if needed, and return the corresponding [`Symbol`].
    ///
    /// If the argument has a `'static` lifetime, use [`get_or_intern_static`](Self::get_or_intern_static) instead.
    pub fn get_or_intern(&self, string: impl AsRef<str>) -> Symbol {
        Symbol(self.0.intern(string.as_ref()))
    }

    /// Intern a static string slice if needed, and return the corresponding [`Symbol`].
    pub fn get_or_intern_static(&self, string: &'static str) -> Symbol {
        Symbol(self.0.intern_static(string))
    }

    /// Resolve a symbol to its interned string slice.
    pub fn resolve(&'static self, sym: Symbol) -> &'static str {
        self.0.resolve(sym.0)
    }
}
