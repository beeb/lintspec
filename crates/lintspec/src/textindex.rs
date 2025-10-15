//! A text index type
//!
//! The [`TextIndex`] type holds the line, column (both zero-indexed) and utf-8/utf-16 offsets for a given position
//! in the text.
use std::{cmp::Ordering, fmt, ops::Range, slice};

use derive_more::Add;
use serde::Serialize;
use wide::{CmpEq as _, CmpLt as _, i8x16};

/// A span of source code
pub type TextRange = Range<TextIndex>;

/// A position inside of the source code
///
/// Lines and columns start at 0.
#[derive(Default, Hash, Copy, Clone, PartialEq, Eq, Debug, Serialize, Add)]
pub struct TextIndex {
    pub utf8: usize,
    pub utf16: usize,
    pub line: usize,
    pub column: usize,
}

impl TextIndex {
    /// Shorthand for `TextIndex { utf8: 0, utf16: 0, line: 0, char: 0 }`.
    pub const ZERO: TextIndex = TextIndex {
        utf8: 0,
        utf16: 0,
        line: 0,
        column: 0,
    };

    /// Advance the index, accounting for lf/nl/ls/ps characters and combinations.
    /// This is *not* derived from the definition of 'newline' in the language definition,
    /// nor is it a complete implementation of the Unicode line breaking algorithm.
    ///
    /// Implementation is directly taken from [`slang_solidity`].
    #[inline]
    pub fn advance(&mut self, c: char, next: Option<&char>) {
        // fast path for ASCII characters
        if c.is_ascii() {
            self.utf8 += 1;
            self.utf16 += 1;
            match (c, next) {
                ('\r', Some(&'\n')) => {
                    // ignore for now, we will increment the line number when we process the \n
                }
                ('\n' | '\r', _) => {
                    self.line += 1;
                    self.column = 0;
                }
                _ => {
                    self.column += 1;
                }
            }
        } else {
            // slow path for Unicode
            self.utf8 += c.len_utf8();
            self.utf16 += c.len_utf16();
            match c {
                '\u{2028}' | '\u{2029}' => {
                    self.line += 1;
                    self.column = 0;
                }
                _ => {
                    self.column += 1;
                }
            }
        }
    }

    /// Advance all offsets by the given number of ASCII `bytes`.
    ///
    /// This function is a shortcut that can be used to hop over spans which contain no newlines and no non-ASCII
    /// characters. The line number is _not_ incremented.
    pub fn advance_by_ascii(&mut self, bytes: usize) {
        self.utf8 += bytes;
        self.utf16 += bytes;
        self.column += bytes;
    }
}

impl fmt::Display for TextIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line + 1, self.column + 1)
    }
}

impl PartialOrd for TextIndex {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TextIndex {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.utf8.cmp(&other.utf8)
    }
}

/// Compute the [`TextIndex`] list corresponding to the byte offsets in the given source.
///
/// The list of offsets _MUST_ be sorted, but it can contain duplicates. The list of offsets _MUST_ have at least 1
/// element. The source string slice _MUST NOT_ be empty.
///
/// This routine iterates through the characters and advances a running [`TextIndex`], storing a copy in the output
/// if it matches a desired offset. Offset zero is always included in the result.
///
/// SIMD is used to accelerate processing of ASCII-only sections in the source.
pub fn compute_indices(source: &str, offsets: &[usize]) -> Vec<TextIndex> {
    assert!(!source.is_empty(), "source cannot be empty");
    let mut text_indices = Vec::with_capacity(offsets.len() + 1); // upper bound for the size
    let mut current = TextIndex::ZERO;
    text_indices.push(current); // just in case zero is needed

    let mut ofs_iter = offsets.iter();
    let mut current_offset = ofs_iter
        .next()
        .expect("there should be one element at least");
    let bytes = source.as_bytes();
    // SAFETY: re-interpreting the u8 slice as i8 slice is memory-safe. All slice invariants are already upheld by
    // the original slice and we use the same pointer and length as the original slice.
    let bytes: &[i8] = unsafe { slice::from_raw_parts(bytes.as_ptr().cast::<i8>(), bytes.len()) };
    'outer: loop {
        while current.utf8 <= bytes.len() - 16
            && let Some(newline_mask) = find_ascii_newlines(&bytes[current.utf8..])
        {
            debug_assert!(current_offset >= &current.utf8);
            let bytes_until_newline = newline_mask.trailing_zeros() as usize;
            if current_offset < &(current.utf8 + 16) {
                // a desired offset is present in this chunk
                let bytes_until_target = (*current_offset).saturating_sub(current.utf8);
                if bytes_until_newline < bytes_until_target {
                    // we hit a newline, need to go into per-char processing routine
                    current.advance_by_ascii(bytes_until_newline);
                    break;
                }
                // else, we reached the target position, store it
                current.advance_by_ascii(bytes_until_target);
                text_indices.push(current);
                // get the next offset of interest, ignoring any duplicates
                current_offset = match ofs_iter.find(|o| o != &current_offset) {
                    Some(o) => o,
                    None => break 'outer, // all interesting offsets have been found
                };
                continue;
            }
            // else, no offset of interest in this chunk
            // fast forward current to before any newline
            current.advance_by_ascii(bytes_until_newline);
            if bytes_until_newline < 16 {
                // we hit a newline, need to go into per-char processing routine
                break;
            }
        }
        // we might have non-ASCII chars in the next 16 chars at `current_byte`
        // we might also have a line ending
        // fall back to character-by-character processing
        let remaining_source = &source[current.utf8..];
        let mut char_iter = remaining_source.chars().peekable();
        let mut found_non_ascii_or_nl = false;
        while let Some(c) = char_iter.next() {
            debug_assert!(current_offset >= &current.utf8);
            current.advance(c, char_iter.peek());
            if !c.is_ascii() || c == '\n' {
                found_non_ascii_or_nl = true;
            }
            match current.utf8.cmp(current_offset) {
                Ordering::Equal => {
                    text_indices.push(current);
                }
                Ordering::Greater => {
                    // skip duplicates and advance to next offset
                    current_offset = match ofs_iter.find(|o| o != &current_offset) {
                        Some(o) => o,
                        None => break 'outer, // all interesting offsets have been found
                    };
                    if current_offset == &current.utf8 {
                        text_indices.push(current);
                    }
                }
                Ordering::Less => {}
            }
            if found_non_ascii_or_nl && char_iter.peek().is_some_and(char::is_ascii) {
                // we're done processing the non-ASCII / newline characters, let's go back to SIMD-optimized processing
                break;
            }
        }
        if current.utf8 >= bytes.len() - 1 {
            break; // done with the input
        }
    }
    text_indices
}

/// Check the first 16 bytes of the input slice for non-ASCII characters and find newline characters.
///
/// If there are non-ASCII characters, the function returns `None`. Otherwise, it returns a mask with bits flipped
/// to 1 for items which correspond to `\n` or `\r`. The least significant bit in the mask corresponds to the first
/// byte in the input.
///
/// This function uses SIMD to accelerate the checks.
fn find_ascii_newlines(chunk: &[i8]) -> Option<u16> {
    let bytes = i8x16::from_slice_unaligned(chunk);

    // check for non-ascii: values 128-255 become i8 values < 0
    let non_ascii_mask = bytes.simd_lt(i8x16::ZERO).to_bitmask();

    if non_ascii_mask != 0 {
        return None;
    }

    // find newlines
    #[allow(clippy::cast_possible_wrap)]
    let lf_bytes = i8x16::splat(b'\n' as i8);
    #[allow(clippy::cast_possible_wrap)]
    let cr_bytes = i8x16::splat(b'\r' as i8);
    let lf_mask = bytes.simd_eq(lf_bytes).to_bitmask();
    let cr_mask = bytes.simd_eq(cr_bytes).to_bitmask();
    let newline_mask = lf_mask | cr_mask;

    #[allow(clippy::cast_possible_truncation)]
    Some(newline_mask as u16)
}
