//! String interner used by data structures.

thread_local! {
    /// The thread-local string interner used by the parsers and other data structures.
    pub static INTERNER: &'static Interner = Box::leak(Box::new(Interner::new()));
}

/// Intern a string reference if needed, and return the corresponding [`Symbol`].
///
/// If the argument has a `'static` lifetime, use [`get_or_intern_static`] instead.
pub fn get_or_intern(s: impl AsRef<str>) -> Symbol {
    INTERNER.with(|i| i.get_or_intern(s))
}

/// Intern a static string slice if needed, and return the corresponding [`Symbol`].
#[expect(clippy::must_use_candidate)]
pub fn get_or_intern_static(s: &'static str) -> Symbol {
    INTERNER.with(|i| i.get_or_intern_static(s))
}

/// Resolve a symbol to its interned string slice.
#[must_use]
pub fn resolve(sym: Symbol) -> &'static str {
    INTERNER.with(|i| i.resolve(sym))
}

/// The reference to an interned string from the [`INTERNER`].
///
/// NOTE: This symbol can only be resolved in the same thread where it was created. This is because the interner is
/// thread-local. The resolved string slice has a static lifetime and can be used in other threads after resolution.
///
/// This type can be compared to rapidly check for string equality, within a single thread.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Symbol(inturn::Symbol);

impl Symbol {
    /// Resolve the symbol to its interned string slice.
    #[must_use]
    pub fn resolve(self) -> &'static str {
        INTERNER.with(|i| i.resolve(self))
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
    #[must_use]
    pub fn resolve(&'static self, sym: Symbol) -> &'static str {
        self.0.resolve(sym.0)
    }
}
