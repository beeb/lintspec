//! A text index type
//!
//! The [`TextIndex`] type holds the line, column (both zero-indexed) and utf-8/utf-16 offsets for a given position
//! in the text.
use std::{fmt, ops::Range, slice};

use derive_more::Add;
use serde::Serialize;
use wide::{CmpEq as _, CmpLt as _, i8x32};

const SIMD_LANES: usize = i8x32::LANES as usize;

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
    ///
    /// This is *not* derived from the definition of 'newline' in the language definition,
    /// nor is it a complete implementation of the Unicode line breaking algorithm.
    ///
    /// Implementation inspired by [`slang_solidity`](https://crates.io/crates/slang_solidity).
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
/// if it matches a desired offset.
///
/// SIMD is used to accelerate processing of ASCII-only sections in the source.
pub fn compute_indices(source: &str, offsets: &[usize]) -> Vec<TextIndex> {
    assert!(!source.is_empty(), "source cannot be empty");
    let mut text_indices = Vec::with_capacity(offsets.len()); // upper bound for the size
    let mut current = TextIndex::ZERO;

    let mut ofs_iter = offsets.iter();
    let mut next_offset = ofs_iter
        .next()
        .expect("there should be one element at least");
    let bytes = source.as_bytes();
    // SAFETY: this is safe as we're re-interpreting a valid slice of u8 as i8.
    // All slice invariants are already upheld by the original slice and we use the same pointer and length as the
    // original slice.
    let bytes: &[i8] = unsafe { slice::from_raw_parts(bytes.as_ptr().cast::<i8>(), bytes.len()) };
    'outer: loop {
        // check whether we can try to process a 16-bytes chunk with SIMD
        while current.utf8 + SIMD_LANES < bytes.len() {
            debug_assert!(next_offset >= &current.utf8);
            let newline_non_ascii_mask = find_non_ascii_and_newlines(&bytes[current.utf8..]);
            let bytes_until_nl_na = newline_non_ascii_mask.trailing_zeros() as usize;
            if bytes_until_nl_na == 0 {
                // we hit a newline or non-ASCII char, need to go into per-char processing routine
                break;
            }
            if next_offset < &(current.utf8 + SIMD_LANES) {
                // a desired offset is present in this chunk
                let bytes_until_target = (*next_offset).saturating_sub(current.utf8);
                if bytes_until_nl_na < bytes_until_target {
                    // we hit a newline or non-ASCII char, need to go into per-char processing routine
                    current.advance_by_ascii(bytes_until_nl_na); // advance because there are ASCII bytes we can process
                    break;
                }
                // else, we reached the target position and it's an ASCII char, store it
                current.advance_by_ascii(bytes_until_target);
                text_indices.push(current);
                // get the next offset of interest, ignoring any duplicates
                next_offset = match ofs_iter.find(|o| o != &next_offset) {
                    Some(o) => o,
                    None => break 'outer, // all interesting offsets have been found
                };
                continue;
            }
            // else, no offset of interest in this chunk
            // fast forward current to before any newline/non-ASCII
            current.advance_by_ascii(bytes_until_nl_na);
            if bytes_until_nl_na < SIMD_LANES {
                // we hit a newline or non-ASCII char, need to go into per-char processing routine
                break;
            }
        }
        if &current.utf8 == next_offset {
            // we reached a target position, store it
            text_indices.push(current);
            // skip duplicates and advance to next offset
            next_offset = match ofs_iter.find(|o| o != &next_offset) {
                Some(o) => o,
                None => break 'outer, // all interesting offsets have been found
            };
        }
        // normally, the next byte is either part of a Unicode char or line ending
        // fall back to character-by-character processing
        let remaining_source = &source[current.utf8..];
        let mut char_iter = remaining_source.chars().peekable();
        let mut found_na_nl = false;
        while let Some(c) = char_iter.next() {
            debug_assert!(
                next_offset >= &current.utf8,
                "next offset {next_offset} is smaller than current {}",
                current.utf8
            );
            current.advance(c, char_iter.peek());
            if !c.is_ascii() || c == '\n' {
                found_na_nl = true;
            }
            if &current.utf8 == next_offset {
                // we reached a target position, store it
                text_indices.push(current);
                // skip duplicates and advance to next offset
                next_offset = match ofs_iter.find(|o| o != &next_offset) {
                    Some(o) => o,
                    None => break 'outer, // all interesting offsets have been found
                };
            }
            if found_na_nl && char_iter.peek().is_some_and(char::is_ascii) {
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

/// Check the first `SIMD_LANES` bytes of the input slice for non-ASCII characters and newline characters.
///
/// The function returns a mask with bits flipped to 1 for items which correspond to `\n` or `\r` or non-ASCII
/// characters. The least significant bit in the mask corresponds to the first byte in the input.
///
/// This function uses SIMD to accelerate the checks.
fn find_non_ascii_and_newlines(chunk: &[i8]) -> u32 {
    let bytes = i8x32::new(
        chunk[..SIMD_LANES]
            .try_into()
            .expect("slice to contain enough bytes"),
    );

    // find non-ASCII
    // u8 values from 128 to 255 correspond to i8 values -128 to -1
    let nonascii_mask = bytes.simd_lt(i8x32::ZERO).to_bitmask();
    // find newlines
    #[allow(clippy::cast_possible_wrap)]
    let lf_bytes = i8x32::splat(b'\n' as i8);
    #[allow(clippy::cast_possible_wrap)]
    let cr_bytes = i8x32::splat(b'\r' as i8);
    let lf_mask = bytes.simd_eq(lf_bytes).to_bitmask();
    let cr_mask = bytes.simd_eq(cr_bytes).to_bitmask();
    // combine masks
    nonascii_mask | lf_mask | cr_mask
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use std::iter::repeat_n;

    use similar_asserts::assert_eq;

    use super::*;

    #[test]
    fn test_find_non_ascii_and_newlines_ascii_only() {
        let chunk: Vec<_> = repeat_n('a', SIMD_LANES).map(|b| b as i8).collect();
        let result = find_non_ascii_and_newlines(&chunk);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_find_non_ascii_and_newlines_with_newline() {
        let chunk: Vec<_> = b"abc\ndef\rghijklmnaaaaaaaaaaaaaaaa"
            .iter()
            .map(|b| *b as i8)
            .collect();
        let result = find_non_ascii_and_newlines(&chunk);
        // \n is at position 3, \r is at position 7
        assert_eq!(result, 1 << 3 | 1 << 7);
    }

    #[test]
    fn test_find_non_ascii_and_newlines_with_non_ascii() {
        let input = "abcd🦀fghijklmnoaaaaaaaaaaaaaaaa";
        let chunk: Vec<_> = input.bytes().map(|b| b as i8).collect();
        let result = find_non_ascii_and_newlines(&chunk);
        // the emoji takes 4 bytes starting at position 4
        assert_eq!(result, 1 << 4 | 1 << 5 | 1 << 6 | 1 << 7);
    }

    #[test]
    fn test_find_non_ascii_and_newlines_mixed() {
        let input = "ab\nc🦀\rfghijklmnopaaaaaaaaaaaaaaaa";
        let chunk: Vec<_> = input.bytes().map(|b| b as i8).collect();
        let result = find_non_ascii_and_newlines(&chunk);
        // line endings at 2 and 8
        // emoji at 4-7
        assert_eq!(result, 1 << 2 | 1 << 4 | 1 << 5 | 1 << 6 | 1 << 7 | 1 << 8);
    }

    #[test]
    fn test_compute_indices_simple() {
        let source = "hello world";
        let offsets = vec![0, 5, 6, 10]; // h, space, w, d
        let result = compute_indices(source, &offsets);

        assert_eq!(result.len(), 4);
        assert_eq!(result[0], TextIndex::ZERO);
        assert_eq!(
            result[1],
            TextIndex {
                utf8: 5,
                utf16: 5,
                line: 0,
                column: 5
            }
        );
        assert_eq!(
            result[2],
            TextIndex {
                utf8: 6,
                utf16: 6,
                line: 0,
                column: 6
            }
        );
        assert_eq!(
            result[3],
            TextIndex {
                utf8: 10,
                utf16: 10,
                line: 0,
                column: 10
            }
        );
    }

    #[test]
    fn test_compute_indices_with_newlines() {
        let source = "hello\nworld\ntest";
        let offsets = vec![0, 5, 6, 12, 15]; // h, nl, w, t, t
        let result = compute_indices(source, &offsets);

        assert_eq!(result.len(), 5);
        assert_eq!(
            result[0],
            TextIndex {
                utf8: 0,
                utf16: 0,
                line: 0,
                column: 0
            }
        );
        assert_eq!(
            result[1],
            TextIndex {
                utf8: 5,
                utf16: 5,
                line: 0,
                column: 5
            }
        );
        assert_eq!(
            result[2],
            TextIndex {
                utf8: 6,
                utf16: 6,
                line: 1,
                column: 0
            }
        );
        assert_eq!(
            result[3],
            TextIndex {
                utf8: 12,
                utf16: 12,
                line: 2,
                column: 0
            }
        );
        assert_eq!(
            result[4],
            TextIndex {
                utf8: 15,
                utf16: 15,
                line: 2,
                column: 3
            }
        );
    }

    #[test]
    fn test_compute_indices_with_unicode() {
        let source = "hel🦀lo";
        let offsets = vec![0, 3, 7]; // h, crab, l
        let result = compute_indices(source, &offsets);

        assert_eq!(
            result[0],
            TextIndex {
                utf8: 0,
                utf16: 0,
                line: 0,
                column: 0
            }
        );
        assert_eq!(
            result[1],
            TextIndex {
                utf8: 3,
                utf16: 3,
                line: 0,
                column: 3
            }
        );
        assert_eq!(
            result[2],
            TextIndex {
                utf8: 7,
                utf16: 5,
                line: 0,
                column: 4
            }
        );
    }

    #[test]
    fn test_compute_indices_with_carriage_return() {
        let source = "padding_hello\r\nworld_padding_padding";
        let offsets = vec![8, 13, 14, 16, 36]; // h, \r, \n, o, g (last)
        let result = compute_indices(source, &offsets);

        assert_eq!(
            result[0],
            TextIndex {
                utf8: 8,
                utf16: 8,
                line: 0,
                column: 8
            }
        );
        assert_eq!(
            result[1],
            TextIndex {
                utf8: 13,
                utf16: 13,
                line: 0,
                column: 13
            }
        );
        assert_eq!(
            result[2],
            TextIndex {
                utf8: 14,
                utf16: 14,
                line: 0,
                column: 13 // \r doesn't advance
            }
        );
        assert_eq!(
            result[3],
            TextIndex {
                utf8: 16,
                utf16: 16,
                line: 1,
                column: 1
            }
        );
        assert_eq!(
            result[4],
            TextIndex {
                utf8: 36,
                utf16: 36,
                line: 1,
                column: 21
            }
        );
    }

    #[test]
    fn test_compute_indices_duplicate_offsets() {
        let source = "hello";
        let offsets = vec![0, 0, 2, 2, 4];
        let result = compute_indices(source, &offsets);

        assert_eq!(result.len(), 3); // duplicates should be handled
        assert_eq!(
            result[0],
            TextIndex {
                utf8: 0,
                utf16: 0,
                line: 0,
                column: 0
            }
        );
        assert_eq!(
            result[1],
            TextIndex {
                utf8: 2,
                utf16: 2,
                line: 0,
                column: 2
            }
        );
        assert_eq!(
            result[2],
            TextIndex {
                utf8: 4,
                utf16: 4,
                line: 0,
                column: 4
            }
        );
    }

    #[test]
    fn test_compute_indices_unicode_line_separators() {
        let source = "hello\u{2028}world\u{2029}test";
        // Unicode line separator (\u{2028}) and paragraph separator (\u{2029}) are 3 bytes in UTF-8
        // and 1 code unit in UTF-16
        let offsets = vec![0, 5, 8, 16]; // h, ls, w, t
        let result = compute_indices(source, &offsets);

        assert_eq!(result[0], TextIndex::ZERO);
        assert_eq!(
            result[1],
            TextIndex {
                utf8: 5,
                utf16: 5,
                line: 0,
                column: 5
            }
        );
        assert_eq!(
            result[2],
            TextIndex {
                utf8: 8,
                utf16: 6,
                line: 1,
                column: 0
            }
        );
        assert_eq!(
            result[3],
            TextIndex {
                utf8: 16,
                utf16: 12,
                line: 2,
                column: 0
            }
        );
    }

    #[test]
    #[should_panic(expected = "source cannot be empty")]
    fn test_compute_indices_empty_source() {
        let source = "";
        let offsets = vec![0];
        compute_indices(source, &offsets);
    }
}
