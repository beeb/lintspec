//! A text index type
//!
//! The [`TextIndex`] type holds the line, column (both zero-indexed) and utf-8/utf-16 offsets for a given position
//! in the text.
use std::{fmt, ops::Range};

use derive_more::Add;
use serde::Serialize;
use wide::{CmpEq as _, CmpLt as _, i8x32};
use zerocopy::transmute_ref;

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

    /// Advance the TextIndex knowing the char `c` is non-ASCII
    #[inline]
    fn advance_unicode(&mut self, c: char) {
        debug_assert!(!c.is_ascii());
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

    /// Advance this index according to the `Advance` parameter.
    #[inline]
    fn advance_by(&mut self, advance: &Advance) {
        self.utf8 += advance.bytes;
        self.utf16 += advance.bytes;
        self.line += advance.lines;
        match advance.column {
            Column::Increment(n) => self.column += n,
            Column::Set(n) => self.column = n,
        }
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

/// The type of operation to perform on the `TextIndex`'s `column` field
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Column {
    Increment(usize),
    Set(usize),
}

/// An update to perform on `TextIndex` after scanning a chunk of the input text
#[derive(Debug, Clone, PartialEq, Eq)]
struct Advance {
    bytes: usize,
    lines: usize,
    column: Column,
}

impl Advance {
    /// Scan a chunk of text and compute how to advance the `TextIndex`
    ///
    /// The return value calculates how much the index can be advanced, until either a non-ASCII character is
    /// encountered, or the next offset of interest is reached.
    #[inline]
    #[must_use]
    fn scan(slice: &[i8], start: usize, next_offset: usize) -> Self {
        let bytes = &slice[start..next_offset];
        let arr: [i8; SIMD_LANES] = bytes.first_chunk().copied().unwrap_or_else(|| {
            // if we have fewer than the required bytes, we pad with `-1` which corresponds to a non-ASCII character
            let mut arr = [-1; SIMD_LANES];
            arr[0..bytes.len()].copy_from_slice(bytes);
            arr
        });
        Self::from(arr)
    }
}

impl From<[i8; SIMD_LANES]> for Advance {
    /// Scan a chunk of text and compute how to advance the `TextIndex`
    ///
    /// The return value calculates how much the index can be advanced, until either a non-ASCII character is
    /// encountered, or the next offset of interest is reached.
    ///
    /// Simplified example with a 16 bytes chunk:
    ///
    /// ```norust
    /// input:          "ab\r\nefg\rijkl🦀"
    /// non-ASCII mask: 0000_0000_0000_1111
    /// LF mask:        0001_0000_0000_0000
    /// CR mask:        0010_0001_0000_0000
    /// to keep:        ^^^^ ^^^^ ^^^^
    /// ```
    ///
    /// We will process an amount of bytes equal to `nonascii_mask.trailing_zeros()` (a contiguous segment of ASCII
    /// bytes at the start of the chunk). This is the increment that will be applied to the `utf8` and `utf16` fields
    /// of `TextIndex` (the bytes/code units offset).
    ///
    /// First we ignore everything starting at the first non-ASCII byte by shifting the masks:
    ///
    /// ```norust
    /// padding:        vvvv
    /// non-ASCII mask: 0000_0000_0000_0000
    /// LF mask:        0000_0001_0000_0000
    /// CR mask:        0000_0010_0001_0000
    /// interesting:         ^^^^ ^^^^ ^^^^
    /// to keep:                  ^^^^ ^^^^
    /// ```
    ///
    /// The number of ASCII bytes on the last line is `lf_mask.leading_zeros()`. The total number of line returns is
    /// `lf_mask.count_ones()`. We will increment the `line` field of `TextIndex` by this number.
    ///
    /// Then we ignore everything but the last line (what comes after the last LF) by shifting the masks in the other
    /// direction:
    ///
    /// ```norust
    /// padding:                  vvvv vvvv
    /// non-ASCII mask: 0000_0000_0000_0000
    /// LF mask:        0000_0000_0000_0000
    /// CR mask:        0001_0000_0000_0000
    /// interesting:    ^^^^ ^^^^
    /// ```
    ///
    /// Finally, we subtract the number of `\r` bytes from the last line (`cr_mask.count_ones()`, which do not
    /// increment the column count) from the number of bytes on the last line which we calculated before. This number
    /// is the new value of the `column` field of `TextIndex`.
    #[inline]
    fn from(chunk: [i8; SIMD_LANES]) -> Self {
        let bytes = i8x32::new(chunk);
        let nonascii_mask = bytes.simd_lt(i8x32::ZERO).to_bitmask();
        #[allow(clippy::cast_possible_wrap)]
        let lf_bytes = i8x32::splat(b'\n' as i8);
        let mut lf_mask = bytes.simd_eq(lf_bytes).to_bitmask();
        #[allow(clippy::cast_possible_wrap)]
        let cr_bytes = i8x32::splat(b'\r' as i8);
        let mut cr_mask = bytes.simd_eq(cr_bytes).to_bitmask();

        // ignore non-ASCII characters at the end
        let n_ascii = nonascii_mask.trailing_zeros() as usize;
        if n_ascii == 0 {
            // there are not ASCII bytes at the start of the chunk
            return Advance {
                bytes: 0,
                column: Column::Increment(0),
                lines: 0,
            };
        }
        let shift = SIMD_LANES - n_ascii; // this is < SIMD_LANES
        lf_mask <<= shift;
        cr_mask <<= shift;

        let mut n_lines = 0;
        let column = if lf_mask > 0 {
            // the chunk contains multiple lines, we ignore everything but the last line
            n_lines = lf_mask.count_ones() as usize;
            let n_last_line = lf_mask.leading_zeros() as usize;
            if n_last_line == 0 {
                // edge case where the last byte is \n
                return Advance {
                    bytes: n_ascii,
                    column: Column::Set(0),
                    lines: n_lines,
                };
            }
            // we ignore the \r in the last line for the columns count
            cr_mask >>= SIMD_LANES - n_last_line; // the shift amount is < SIMD_LANES
            Column::Set(n_last_line - cr_mask.count_ones() as usize)
        } else {
            Column::Increment(n_ascii - cr_mask.count_ones() as usize)
        };
        Advance {
            bytes: n_ascii,
            column,
            lines: n_lines,
        }
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
    // need to cast the bytes to `i8` to work with SIMD instructions from `wide`.
    let bytes: &[i8] = transmute_ref!(source.as_bytes());
    'outer: loop {
        // process a chunk with SIMD
        loop {
            let advance = Advance::scan(bytes, current.utf8, *next_offset);
            current.advance_by(&advance);
            if &current.utf8 == next_offset {
                // we reached a target position, store it
                text_indices.push(current);
                // skip duplicates and advance to next offset
                next_offset = match ofs_iter.find(|o| o != &next_offset) {
                    Some(o) => o,
                    None => break 'outer, // all interesting offsets have been found
                };
            }
            if bytes[current.utf8] < 0 {
                // a non-ASCII character was found, let's go to char-by-char processing
                break;
            }
        }
        // fall back to char-by-char processing
        let remaining_source = &source[current.utf8..];
        let mut char_iter = remaining_source.chars().peekable();
        while let Some(c) = char_iter.next() {
            debug_assert!(
                next_offset >= &current.utf8,
                "next offset {next_offset} is smaller than current {}",
                current.utf8
            );
            current.advance_unicode(c);
            if &current.utf8 >= next_offset {
                // we reached a target position, store it
                // for offsets that fall in the middle of a unicode character, we store the next valid position
                text_indices.push(current);
                // skip duplicates and advance to next offset
                next_offset = match ofs_iter.find(|o| o != &next_offset) {
                    Some(o) => o,
                    None => break 'outer, // all interesting offsets have been found
                };
            }
            if char_iter.peek().is_some_and(char::is_ascii) {
                // we're done processing the non-ASCII characters, let's go back to SIMD-optimized processing
                break;
            }
        }
        if current.utf8 >= bytes.len() - 1 {
            break; // done with the input
        }
    }
    text_indices
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use similar_asserts::assert_eq;

    use super::*;

    #[test]
    fn test_advance_simple() {
        let chunk: Vec<_> = b"abcdabcdabcdabcdabcdabcdabcdabcd"
            .iter()
            .map(|b| *b as i8)
            .collect();
        let chunk: [i8; 32] = chunk.as_slice().try_into().unwrap();
        let advance = Advance::from(chunk);
        assert_eq!(
            advance,
            Advance {
                bytes: 32,
                lines: 0,
                column: Column::Increment(32)
            }
        );
    }

    #[test]
    fn test_advance_newline() {
        let chunk: Vec<_> = b"abcdabcdabcdabcdabcdabcdabc\nabcd"
            .iter()
            .map(|b| *b as i8)
            .collect();
        let chunk: [i8; 32] = chunk.as_slice().try_into().unwrap();
        let advance = Advance::from(chunk);
        assert_eq!(
            advance,
            Advance {
                bytes: 32,
                lines: 1,
                column: Column::Set(4)
            }
        );
    }

    #[test]
    fn test_advance_multiple_newlines() {
        let chunk: Vec<_> = b"abcdabcdabc\nabcdabcdabcdab\r\nabc\r"
            .iter()
            .map(|b| *b as i8)
            .collect();
        let chunk: [i8; 32] = chunk.as_slice().try_into().unwrap();
        let advance = Advance::from(chunk);
        assert_eq!(
            advance,
            Advance {
                bytes: 32,
                lines: 2,
                column: Column::Set(3)
            }
        );
    }

    #[test]
    fn test_advance_unicode() {
        let chunk: Vec<_> = "abcdabcdabcdabcdabcdabcdabcd🦀"
            .bytes()
            .map(|b| b as i8)
            .collect();
        let chunk: [i8; 32] = chunk.as_slice().try_into().unwrap();
        let advance = Advance::from(chunk);
        assert_eq!(
            advance,
            Advance {
                bytes: 28,
                lines: 0,
                column: Column::Increment(28)
            }
        );
    }

    #[test]
    fn test_advance_unicode_newlines() {
        let chunk: Vec<_> = "abcdabcdabc\nabcdabcdabc\nabcd🦀"
            .bytes()
            .map(|b| b as i8)
            .collect();
        let chunk: [i8; 32] = chunk.as_slice().try_into().unwrap();
        let advance = Advance::from(chunk);
        assert_eq!(
            advance,
            Advance {
                bytes: 28,
                lines: 2,
                column: Column::Set(4)
            }
        );
    }

    #[test]
    fn test_advance_next_offset() {
        let chunk: Vec<_> = b"abcdabcdabcdabcdabcdabcdabcdabcd"
            .iter()
            .map(|b| *b as i8)
            .collect();
        let advance = Advance::scan(chunk.as_slice(), 0, 28);
        assert_eq!(
            advance,
            Advance {
                bytes: 28,
                lines: 0,
                column: Column::Increment(28)
            }
        );
    }

    #[test]
    fn test_advance_next_offset_newline() {
        let chunk: Vec<_> = b"abcdabcdabcdabc\nabcdabcdabc\nabcd"
            .iter()
            .map(|b| *b as i8)
            .collect();
        let advance = Advance::scan(chunk.as_slice(), 0, 28);
        assert_eq!(
            advance,
            Advance {
                bytes: 28,
                lines: 2,
                column: Column::Set(0)
            }
        );
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
