//! Source positions as TypeScript's API reports them.
//!
//! surge's parsers record UTF-8 byte offsets; TypeScript's API speaks in
//! UTF-16 code units, the length JavaScript strings have. Every position the
//! API hands out goes through a [`PositionMap`], and lines follow tsc's
//! `computeLineStarts` (CR, LF, CRLF, U+2028, U+2029).

/// Converts between byte offsets and UTF-16 offsets in one text.
#[derive(Debug, Default)]
pub(crate) struct PositionMap {
    /// `(byte offset, UTF-16 offset)` just after each non-ASCII character.
    /// Between checkpoints every character is one byte and one code unit.
    checkpoints: Vec<(u32, u32)>,
    byte_len: u32,
    utf16_len: u32,
    /// UTF-16 offsets of every line start.
    line_starts: Vec<u32>,
}

impl PositionMap {
    pub(crate) fn new(text: &str) -> Self {
        let mut checkpoints = Vec::new();
        let mut line_starts = vec![0];
        let mut utf16 = 0u32;
        let mut chars = text.char_indices().peekable();
        while let Some((byte, ch)) = chars.next() {
            let units = ch.len_utf16() as u32;
            utf16 += units;
            match ch {
                '\r' => {
                    if let Some((_, '\n')) = chars.peek() {
                        chars.next();
                        utf16 += 1;
                    }
                    line_starts.push(utf16);
                }
                '\n' | '\u{2028}' | '\u{2029}' => line_starts.push(utf16),
                _ => {}
            }
            if !ch.is_ascii() {
                checkpoints.push(((byte + ch.len_utf8()) as u32, utf16));
            }
        }
        PositionMap {
            checkpoints,
            byte_len: text.len() as u32,
            utf16_len: utf16,
            line_starts,
        }
    }

    pub(crate) fn utf16_len(&self) -> u32 {
        self.utf16_len
    }

    /// The UTF-16 offset of a byte offset (a character boundary).
    pub(crate) fn to_utf16(&self, byte: usize) -> u32 {
        let byte = (byte as u32).min(self.byte_len);
        match self.checkpoints.partition_point(|&(checkpoint, _)| checkpoint <= byte) {
            0 => byte,
            index => {
                let (checkpoint_byte, checkpoint_utf16) = self.checkpoints[index - 1];
                checkpoint_utf16 + (byte - checkpoint_byte)
            }
        }
    }

    /// The byte offset of a UTF-16 offset. An offset inside a surrogate pair
    /// resolves to the start of its character.
    #[cfg(test)]
    fn to_byte(&self, utf16: u32) -> usize {
        let utf16 = utf16.min(self.utf16_len);
        match self.checkpoints.partition_point(|&(_, checkpoint)| checkpoint <= utf16) {
            0 => utf16 as usize,
            index => {
                let (checkpoint_byte, checkpoint_utf16) = self.checkpoints[index - 1];
                (checkpoint_byte + (utf16 - checkpoint_utf16)) as usize
            }
        }
    }

    pub(crate) fn line_starts(&self) -> &[u32] {
        &self.line_starts
    }

    /// tsc's `computeLineAndCharacterOfPosition`, both zero-based.
    pub(crate) fn line_and_character(&self, position: u32) -> LineAndCharacter {
        let line = self.line_starts.partition_point(|&start| start <= position).saturating_sub(1);
        LineAndCharacter {
            line: line as u32,
            character: position - self.line_starts[line],
        }
    }

    /// tsc's `computePositionOfLineAndCharacter`: `None` for a line the text
    /// does not have or a character past its line's end.
    pub(crate) fn position_of(&self, line: u32, character: u32) -> Option<u32> {
        let start = *self.line_starts.get(line as usize)?;
        let position = start.checked_add(character)?;
        let limit = match self.line_starts.get(line as usize + 1) {
            Some(&next) => next,
            None => self.utf16_len + 1,
        };
        (position < limit).then_some(position)
    }
}

/// A zero-based line and UTF-16 character offset within it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineAndCharacter {
    pub line: u32,
    pub character: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_text_maps_identically() {
        let map = PositionMap::new("let x = 1;\nx;");
        assert_eq!(map.to_utf16(5), 5);
        assert_eq!(map.to_byte(5), 5);
        assert_eq!(map.line_and_character(11), LineAndCharacter { line: 1, character: 0 });
    }

    #[test]
    fn non_ascii_characters_count_utf16_units() {
        // 'é' is 2 bytes and 1 unit; '😀' is 4 bytes and 2 units.
        let text = "é😀x";
        let map = PositionMap::new(text);
        assert_eq!(map.to_utf16(0), 0);
        assert_eq!(map.to_utf16(2), 1);
        assert_eq!(map.to_utf16(6), 3);
        assert_eq!(map.to_utf16(7), 4);
        assert_eq!(map.utf16_len(), 4);
        assert_eq!(map.to_byte(3), 6);
        assert_eq!(map.to_byte(4), 7);
    }

    #[test]
    fn line_starts_follow_tsc() {
        let map = PositionMap::new("a\r\nb\rc\u{2028}d\ne");
        assert_eq!(map.line_starts(), &[0, 3, 5, 7, 9]);
        assert_eq!(map.position_of(2, 1), Some(6));
        assert_eq!(map.position_of(2, 2), None);
        assert_eq!(map.position_of(4, 1), Some(10));
        assert_eq!(map.position_of(5, 0), None);
    }
}
