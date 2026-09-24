//! typescript-go's `stringutil` character classes. A "rune" is an `i32` so the
//! scanner can use `-1` for end of text as Go does.

pub const EOF: i32 = -1;
/// `utf8.RuneError`: what Go decodes a byte sequence that is not UTF-8 to.
pub const RUNE_ERROR: i32 = 0xFFFD;

pub fn is_white_space_single_line(ch: i32) -> bool {
    matches!(
        ch,
        0x20 | 0x09 | 0x0B | 0x0C | 0x85 | 0xA0 | 0x1680 | 0x2000..=0x200B | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

pub fn is_line_break(ch: i32) -> bool {
    matches!(ch, 0x0A | 0x0D | 0x2028 | 0x2029)
}

pub fn is_white_space_like(ch: i32) -> bool {
    is_white_space_single_line(ch) || is_line_break(ch)
}

pub fn is_digit(ch: i32) -> bool {
    (b'0' as i32..=b'9' as i32).contains(&ch)
}

pub fn is_octal_digit(ch: i32) -> bool {
    (b'0' as i32..=b'7' as i32).contains(&ch)
}

pub fn is_hex_digit(ch: i32) -> bool {
    is_digit(ch) || (b'A' as i32..=b'F' as i32).contains(&ch) || (b'a' as i32..=b'f' as i32).contains(&ch)
}

pub fn is_ascii_letter(ch: i32) -> bool {
    (b'A' as i32..=b'Z' as i32).contains(&ch) || (b'a' as i32..=b'z' as i32).contains(&ch)
}

fn as_char(ch: i32) -> Option<char> {
    u32::try_from(ch).ok().and_then(char::from_u32)
}

pub fn is_identifier_start(ch: i32) -> bool {
    is_ascii_letter(ch)
        || ch == '_' as i32
        || ch == '$' as i32
        || ch >= 0x80 && as_char(ch).is_some_and(unicode_id_start::is_id_start)
}

fn is_word_character(ch: i32) -> bool {
    is_ascii_letter(ch) || is_digit(ch) || ch == '_' as i32
}

pub fn is_identifier_part(ch: i32) -> bool {
    is_identifier_part_ex(ch, false)
}

pub fn is_identifier_part_ex(ch: i32, jsx: bool) -> bool {
    is_word_character(ch)
        || ch == '$' as i32
        || ch >= 0x80 && as_char(ch).is_some_and(unicode_id_start::is_id_continue)
        || jsx && (ch == '-' as i32 || ch == ':' as i32)
}

pub fn is_high_surrogate(ch: i32) -> bool {
    (0xD800..0xDC00).contains(&ch)
}

pub fn is_low_surrogate(ch: i32) -> bool {
    (0xDC00..0xE000).contains(&ch)
}

pub fn surrogate_pair_to_code_point(high: i32, low: i32) -> i32 {
    ((high - 0xD800) << 10 | (low - 0xDC00)) + 0x10000
}

/// Go appends `string(rune)`; a code point Rust cannot hold (a lone surrogate,
/// or out of range) becomes U+FFFD, which only token values ever see.
pub fn push_rune(out: &mut String, ch: i32) {
    out.push(as_char(ch).unwrap_or('\u{FFFD}'));
}
