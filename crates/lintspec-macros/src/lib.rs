use quote::{format_ident, quote};
use unsynn::{
    BraceGroupContaining, BracketGroupContaining, CommaDelimitedVec, Cons, Error, Ident,
    LiteralString, Many, Optional, ParenthesisGroupContaining, Parse as _, PathSep,
    PathSepDelimited, Pound, ToTokens as _, TokenIter, TokenStream, TokenTree, Transaction, unsynn,
};

/// Represents a module path, consisting of an optional path separator followed by
/// a path-separator-delimited sequence of identifiers.
type ModPath = Cons<Option<PathSep>, PathSepDelimited<Ident>>;

unsynn! {
    operator Eq = "=";
    keyword EnumKeyword = "enum";
    keyword DocKeyword = "doc";
    keyword ReprKeyword = "repr";
    keyword PubKeyword = "pub";
    keyword InKeyword = "in";

    /// Represents documentation for an item.
    struct DocInner {
        /// The "doc" keyword.
        _kw_doc: DocKeyword,
        /// The equality operator.
        _eq: Eq,
        /// The documentation content as a literal string.
        value: LiteralString,
    }

    /// Represents the inner content of a `repr` attribute, typically used for specifying
    /// memory layout or representation hints.
    struct ReprInner {
        /// The "repr" keyword.
        _kw_repr: ReprKeyword,
        /// The representation attributes enclosed in parentheses.
        attr: ParenthesisGroupContaining<CommaDelimitedVec<Ident>>,
    }

    /// Represents the inner content of an attribute annotation.
    enum AttributeInner {
        /// A documentation attribute typically used for generating documentation.
        Doc(DocInner),
        /// A representation attribute that specifies how data should be laid out.
        Repr(ReprInner),
        /// Any other attribute represented as a sequence of token trees.
        Any(Many<TokenTree>),
    }

    /// Represents an attribute annotation on a field, typically in the form `#[attr]`.
    struct Attribute {
        /// The pound sign preceding the attribute.
        _pound: Pound,
        /// The content of the attribute enclosed in square brackets.
        body: BracketGroupContaining<AttributeInner>,
    }

    /// Represents visibility modifiers for items.
    enum Vis {
        /// `pub(in? crate::foo::bar)`/`pub(in? ::foo::bar)`
        PubIn(Cons<PubKeyword, ParenthesisGroupContaining<Cons<Option<InKeyword>, ModPath>>>),
        /// Public visibility, indicated by the "pub" keyword.
        Pub(PubKeyword),
    }

    /// Represents a simple enum variant.
    struct EnumVariant {
        /// The discriminant
        name: Ident,
        /// The type contained inside of the variant
        body: ParenthesisGroupContaining<Ident>,
    }

    /// Represents an enum with simple variants.
    struct SimpleEnum {
        /// Optional attributes (docs, repr, etc.)
        _attributes: Optional<Many<Attribute>>,
        /// Optional visibility
        _vis: Optional<Vis>,
        /// The "enum" keyword
        _enum_token: EnumKeyword,
        /// The name of the enum
        name: Ident,
        /// The contents of the enum body
        body: BraceGroupContaining<CommaDelimitedVec<EnumVariant>>,
    }
}

/// Derive `as_variant -> Option<&InnerType>` and `to_variant -> Option<InnerType>` for an enum with simple variants
/// in the form `Variant(InnerType)`.
#[proc_macro_derive(AsToVariant)]
pub fn derive_as_to_variant(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: TokenStream = input.into();
    let mut it = input.to_token_iter();

    let enum_def = match SimpleEnum::parse(&mut it) {
        Ok(def) => def,
        Err(e) => panic!("failed to parse enum definition: {e:#?}"),
    };

    let enum_name = enum_def.name;
    let variants = enum_def.body.content;

    // Generate methods for each variant
    let variant_methods: Vec<_> = variants
        .into_iter()
        .map(|variant| {
            let variant_name = &variant.value.name;
            let variant_name_snake = to_snake_case(&variant.value.name.to_string());
            let to_method = format_ident!("to_{variant_name_snake}");
            let as_method = format_ident!("as_{variant_name_snake}");
            let inner_type = &variant.value.body.content;

            quote! {
                /// Convert to the inner #variant_name_lowercase definition
                #[must_use]
                pub fn #to_method(self) -> Option<#inner_type> {
                    match self {
                        #enum_name::#variant_name(value) => Some(value),
                        _ => None,
                    }
                }
                /// Reference to the inner #variant_name_lowercase definition
                #[must_use]
                pub fn #as_method(&self) -> Option<&#inner_type> {
                    match self {
                        #enum_name::#variant_name(value) => Some(value),
                        _ => None,
                    }
                }
            }
        })
        .collect();

    let expanded = quote! {
        impl #enum_name {
            #(#variant_methods)*
        }
    };

    proc_macro::TokenStream::from(expanded)
}

/// Converts a string to `s̀nake_case`: `FooBar` -> `foo_bar`
fn to_snake_case(input: &str) -> String {
    let words = split_into_words(input);
    words
        .iter()
        .map(|word| word.to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

/// Splits a string into words based on case and separators
///
/// Logic:
/// - Iterates through characters in the input string.
/// - Splits at underscores, hyphens, or whitespace.
/// - Starts a new word on case boundaries, e.g. between lowercase and uppercase (as in "fooBar").
/// - Handles consecutive uppercase letters correctly (e.g. `HTTPServer`).
/// - Aggregates non-separator characters into words.
/// - Returns a vector of non-empty words as Strings.
fn split_into_words(input: &str) -> Vec<String> {
    if input.is_empty() {
        return vec![];
    }

    let mut words = Vec::new();
    let mut current_word = String::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        // If separator, start new word
        if c == '_' || c == '-' || c.is_whitespace() {
            if !current_word.is_empty() {
                words.push(std::mem::take(&mut current_word));
            }
            continue;
        }

        // Peek at next character for deciding about word boundaries
        let next = chars.peek().copied();

        if c.is_uppercase() {
            if !current_word.is_empty() {
                let prev = current_word.chars().last().unwrap();
                // Both cases should take the same action, so fold them together.
                // Case 1: previous is lowercase or digit, now uppercase (e.g. fooBar, foo1Bar)
                // Case 2: end of consecutive uppercase group, e.g. "BARBaz"
                // (prev is uppercase and next char is lowercase)
                if prev.is_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_uppercase() && next.is_some_and(char::is_lowercase))
                {
                    words.push(std::mem::take(&mut current_word));
                }
            }
            current_word.push(c);
        } else {
            // Lowercase or digit, just append
            // If previous is uppercase and next is lowercase, need to split, but handled above
            current_word.push(c);
        }
    }

    if !current_word.is_empty() {
        words.push(current_word);
    }

    words.into_iter().filter(|s| !s.is_empty()).collect()
}
