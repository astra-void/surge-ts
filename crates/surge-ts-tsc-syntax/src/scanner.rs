//! typescript-go's `scanner.Scanner`, without JSDoc and regular-expression
//! validation (the parser reports neither as syntactic diagnostics).

use crate::chars::{self, EOF, RUNE_ERROR};
use crate::flags::TokenFlags;
use crate::kind::Kind;
use crate::messages as diagnostics;
use crate::{Diagnostic, Message};

#[derive(Clone, Default)]
pub struct ScannerState {
    pub pos: usize,
    pub full_start_pos: usize,
    pub token_start: usize,
    pub token: Kind,
    pub token_value: String,
    pub token_flags: TokenFlags,
}

pub struct Scanner<'a> {
    text: &'a str,
    end: usize,
    pub jsx: bool,
    skip_trivia: bool,
    state: ScannerState,
    /// The parser's diagnostics: scan errors go straight into them, with the
    /// parser's same-position dedup (`parseErrorAtRange`).
    pub diagnostics: Vec<Diagnostic>,
    pub has_parse_error: bool,
}

impl<'a> Scanner<'a> {
    pub fn new(text: &'a str, jsx: bool) -> Self {
        Self {
            text,
            end: text.len(),
            jsx,
            skip_trivia: true,
            state: ScannerState::default(),
            diagnostics: Vec::new(),
            has_parse_error: false,
        }
    }


    pub fn token(&self) -> Kind {
        self.state.token
    }

    pub fn token_flags(&self) -> TokenFlags {
        self.state.token_flags
    }

    pub fn token_full_start(&self) -> usize {
        self.state.full_start_pos
    }

    pub fn token_start(&self) -> usize {
        self.state.token_start
    }

    pub fn token_end(&self) -> usize {
        self.state.pos
    }

    pub fn token_text(&self) -> &'a str {
        &self.text[self.state.token_start..self.state.pos]
    }

    pub fn token_value(&self) -> &str {
        &self.state.token_value
    }

    pub fn mark(&self) -> ScannerState {
        self.state.clone()
    }

    pub fn rewind(&mut self, state: ScannerState) {
        self.state = state;
    }

    pub fn reset_pos(&mut self, pos: usize) {
        self.state.pos = pos;
        self.state.full_start_pos = pos;
        self.state.token_start = pos;
    }


    pub fn has_unicode_escape(&self) -> bool {
        self.state.token_flags.has(TokenFlags::UnicodeEscape)
    }

    pub fn has_extended_unicode_escape(&self) -> bool {
        self.state.token_flags.has(TokenFlags::ExtendedUnicodeEscape)
    }

    pub fn has_preceding_line_break(&self) -> bool {
        self.state.token_flags.has(TokenFlags::PrecedingLineBreak)
    }

    /// `parseErrorAtRange`: no second error at the position of the last one.
    pub fn error_at_range(&mut self, pos: usize, end: usize, message: &'static Message, args: Vec<String>) {
        if self.diagnostics.last().is_none_or(|last| last.start != pos) {
            self.diagnostics.push(Diagnostic { start: pos, end, message, args });
        }
        self.has_parse_error = true;
    }

    fn error(&mut self, message: &'static Message) {
        self.error_at(message, self.state.pos, 0, Vec::new());
    }

    fn error_at(&mut self, message: &'static Message, pos: usize, length: usize, args: Vec<String>) {
        self.error_at_range(pos, pos + length, message, args);
    }

    fn byte_at(&self, pos: usize) -> i32 {
        if pos < self.end { self.text.as_bytes()[pos] as i32 } else { EOF }
    }

    /// Go's `char()`: the byte at the position, not the decoded character.
    fn char(&self) -> i32 {
        self.byte_at(self.state.pos)
    }

    fn char_at(&self, offset: usize) -> i32 {
        self.byte_at(self.state.pos + offset)
    }

    fn char_and_size(&self) -> (i32, usize) {
        decode_rune(self.text, self.state.pos, self.end)
    }

    fn scan_ascii_while(&mut self, pred: impl Fn(u8) -> bool) {
        let bytes = self.text.as_bytes();
        while self.state.pos < self.end {
            let b = bytes[self.state.pos];
            if b >= 0x80 || !pred(b) {
                break;
            }
            self.state.pos += 1;
        }
    }

    pub fn scan(&mut self) -> Kind {
        self.state.full_start_pos = self.state.pos;
        self.state.token_flags = TokenFlags::None;
        loop {
            let ch = self.char();
            self.state.token_start = self.state.pos;
            let c = if ch >= 0 { ch as u8 } else { 0 };
            match (ch, c) {
                (_, b'\t' | 0x0B | 0x0C | b' ') if ch >= 0 => {
                    self.state.pos += 1;
                    if self.skip_trivia {
                        continue;
                    }
                    loop {
                        let (ch, size) = self.char_and_size();
                        if !chars::is_white_space_single_line(ch) {
                            break;
                        }
                        self.state.pos += size;
                    }
                    self.state.token = Kind::WhitespaceTrivia;
                }
                (_, b'\n' | b'\r') if ch >= 0 => {
                    self.state.token_flags |= TokenFlags::PrecedingLineBreak;
                    if self.skip_trivia {
                        self.state.pos += 1;
                        self.scan_ascii_while(|b| b == b' ' || (b'\t'..=b'\r').contains(&b));
                        continue;
                    }
                    if c == b'\r' && self.char_at(1) == '\n' as i32 {
                        self.state.pos += 2;
                    } else {
                        self.state.pos += 1;
                    }
                    self.state.token = Kind::NewLineTrivia;
                }
                (_, b'!') if ch >= 0 => {
                    if self.char_at(1) == '=' as i32 {
                        if self.char_at(2) == '=' as i32 {
                            self.state.pos += 3;
                            self.state.token = Kind::ExclamationEqualsEqualsToken;
                        } else {
                            self.state.pos += 2;
                            self.state.token = Kind::ExclamationEqualsToken;
                        }
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::ExclamationToken;
                    }
                }
                (_, b'"' | b'\'') if ch >= 0 => {
                    self.state.token_value = self.scan_string(false);
                    self.state.token = Kind::StringLiteral;
                }
                (_, b'`') if ch >= 0 => {
                    self.state.token = self.scan_template_and_set_token_value(false);
                }
                (_, b'%') if ch >= 0 => self.two('=', Kind::PercentEqualsToken, Kind::PercentToken),
                (_, b'&') if ch >= 0 => {
                    let next = self.char_at(1);
                    if next == '&' as i32 {
                        if self.char_at(2) == '=' as i32 {
                            self.state.pos += 3;
                            self.state.token = Kind::AmpersandAmpersandEqualsToken;
                        } else {
                            self.state.pos += 2;
                            self.state.token = Kind::AmpersandAmpersandToken;
                        }
                    } else if next == '=' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::AmpersandEqualsToken;
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::AmpersandToken;
                    }
                }
                (_, b'(') if ch >= 0 => self.one(Kind::OpenParenToken),
                (_, b')') if ch >= 0 => self.one(Kind::CloseParenToken),
                (_, b'*') if ch >= 0 => {
                    let next = self.char_at(1);
                    if next == '=' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::AsteriskEqualsToken;
                    } else if next == '*' as i32 {
                        if self.char_at(2) == '=' as i32 {
                            self.state.pos += 3;
                            self.state.token = Kind::AsteriskAsteriskEqualsToken;
                        } else {
                            self.state.pos += 2;
                            self.state.token = Kind::AsteriskAsteriskToken;
                        }
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::AsteriskToken;
                    }
                }
                (_, b'+') if ch >= 0 => {
                    let next = self.char_at(1);
                    if next == '=' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::PlusEqualsToken;
                    } else if next == '+' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::PlusPlusToken;
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::PlusToken;
                    }
                }
                (_, b',') if ch >= 0 => self.one(Kind::CommaToken),
                (_, b'-') if ch >= 0 => {
                    let next = self.char_at(1);
                    if next == '=' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::MinusEqualsToken;
                    } else if next == '-' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::MinusMinusToken;
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::MinusToken;
                    }
                }
                (_, b'.') if ch >= 0 => {
                    let next = self.char_at(1);
                    if chars::is_digit(next) {
                        self.state.token = self.scan_number();
                    } else if next == '.' as i32 && self.char_at(2) == '.' as i32 {
                        self.state.pos += 3;
                        self.state.token = Kind::DotDotDotToken;
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::DotToken;
                    }
                }
                (_, b'/') if ch >= 0 => {
                    if self.char_at(1) == '/' as i32 {
                        self.state.pos += 2;
                        loop {
                            self.scan_ascii_while(|b| b != b'\n' && b != b'\r');
                            let (ch1, size) = self.char_and_size();
                            if size == 0 || chars::is_line_break(ch1) {
                                break;
                            }
                            self.state.pos += size;
                        }
                        if self.skip_trivia {
                            continue;
                        }
                        self.state.token = Kind::SingleLineCommentTrivia;
                        return self.state.token;
                    }
                    if self.char_at(1) == '*' as i32 {
                        self.state.pos += 2;
                        let mut comment_closed = false;
                        loop {
                            self.scan_ascii_while(|b| b != b'*' && b != b'\n' && b != b'\r');
                            let (ch1, size) = self.char_and_size();
                            if size == 0 {
                                break;
                            }
                            if ch1 == '*' as i32 && self.char_at(1) == '/' as i32 {
                                self.state.pos += 2;
                                comment_closed = true;
                                break;
                            }
                            self.state.pos += size;
                            if chars::is_line_break(ch1) {
                                self.state.token_flags |= TokenFlags::PrecedingLineBreak;
                            }
                        }
                        if !comment_closed {
                            self.error(diagnostics::Asterisk_Slash_expected);
                        }
                        if self.skip_trivia {
                            continue;
                        }
                        if !comment_closed {
                            self.state.token_flags |= TokenFlags::Unterminated;
                        }
                        self.state.token = Kind::MultiLineCommentTrivia;
                        return self.state.token;
                    }
                    self.two('=', Kind::SlashEqualsToken, Kind::SlashToken);
                }
                (_, b'0'..=b'9') if ch >= 0 => {
                    if c == b'0' && (self.char_at(1) == 'X' as i32 || self.char_at(1) == 'x' as i32) {
                        let start = self.state.pos;
                        self.state.pos += 2;
                        let mut digits = self.scan_hex_digits(1, true, true);
                        if digits.is_empty() {
                            self.error(diagnostics::Hexadecimal_digit_expected);
                            digits = "0".to_string();
                        }
                        let raw = &self.text[start..self.state.pos];
                        self.state.token_value = if raw.starts_with("0x") && raw[2..] == digits {
                            raw.to_string()
                        } else {
                            format!("0x{digits}")
                        };
                        self.state.token_flags |= TokenFlags::HexSpecifier;
                        self.state.token = self.scan_big_int_suffix();
                    } else if c == b'0' && (self.char_at(1) == 'B' as i32 || self.char_at(1) == 'b' as i32) {
                        self.state.pos += 2;
                        let mut digits = self.scan_binary_or_octal_digits(2);
                        if digits.is_empty() {
                            self.error(diagnostics::Binary_digit_expected);
                            digits = "0".to_string();
                        }
                        self.state.token_value = format!("0b{digits}");
                        self.state.token_flags |= TokenFlags::BinarySpecifier;
                        self.state.token = self.scan_big_int_suffix();
                    } else if c == b'0' && (self.char_at(1) == 'O' as i32 || self.char_at(1) == 'o' as i32) {
                        self.state.pos += 2;
                        let mut digits = self.scan_binary_or_octal_digits(8);
                        if digits.is_empty() {
                            self.error(diagnostics::Octal_digit_expected);
                            digits = "0".to_string();
                        }
                        self.state.token_value = format!("0o{digits}");
                        self.state.token_flags |= TokenFlags::OctalSpecifier;
                        self.state.token = self.scan_big_int_suffix();
                    } else {
                        self.state.token = self.scan_number();
                    }
                }
                (_, b':') if ch >= 0 => self.one(Kind::ColonToken),
                (_, b';') if ch >= 0 => self.one(Kind::SemicolonToken),
                (_, b'<') if ch >= 0 => {
                    if self.char_at(1) == '<' as i32 && is_conflict_marker_trivia(self.text, self.state.pos) {
                        self.state.pos = self.scan_conflict_marker_trivia(self.state.pos);
                        if self.skip_trivia {
                            continue;
                        }
                        self.state.token = Kind::ConflictMarkerTrivia;
                        return self.state.token;
                    }
                    if self.char_at(1) == '<' as i32 {
                        if self.char_at(2) == '=' as i32 {
                            self.state.pos += 3;
                            self.state.token = Kind::LessThanLessThanEqualsToken;
                        } else {
                            self.state.pos += 2;
                            self.state.token = Kind::LessThanLessThanToken;
                        }
                    } else if self.char_at(1) == '=' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::LessThanEqualsToken;
                    } else if self.jsx && self.char_at(1) == '/' as i32 && self.char_at(2) != '*' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::LessThanSlashToken;
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::LessThanToken;
                    }
                }
                (_, b'=') if ch >= 0 => {
                    if self.char_at(1) == '=' as i32 && is_conflict_marker_trivia(self.text, self.state.pos) {
                        self.state.pos = self.scan_conflict_marker_trivia(self.state.pos);
                        if self.skip_trivia {
                            continue;
                        }
                        self.state.token = Kind::ConflictMarkerTrivia;
                        return self.state.token;
                    }
                    if self.char_at(1) == '=' as i32 {
                        if self.char_at(2) == '=' as i32 {
                            self.state.pos += 3;
                            self.state.token = Kind::EqualsEqualsEqualsToken;
                        } else {
                            self.state.pos += 2;
                            self.state.token = Kind::EqualsEqualsToken;
                        }
                    } else if self.char_at(1) == '>' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::EqualsGreaterThanToken;
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::EqualsToken;
                    }
                }
                (_, b'>') if ch >= 0 => {
                    if self.char_at(1) == '>' as i32 && is_conflict_marker_trivia(self.text, self.state.pos) {
                        self.state.pos = self.scan_conflict_marker_trivia(self.state.pos);
                        if self.skip_trivia {
                            continue;
                        }
                        self.state.token = Kind::ConflictMarkerTrivia;
                        return self.state.token;
                    }
                    self.one(Kind::GreaterThanToken);
                }
                (_, b'?') if ch >= 0 => {
                    if self.char_at(1) == '.' as i32 && !chars::is_digit(self.char_at(2)) {
                        self.state.pos += 2;
                        self.state.token = Kind::QuestionDotToken;
                    } else if self.char_at(1) == '?' as i32 {
                        if self.char_at(2) == '=' as i32 {
                            self.state.pos += 3;
                            self.state.token = Kind::QuestionQuestionEqualsToken;
                        } else {
                            self.state.pos += 2;
                            self.state.token = Kind::QuestionQuestionToken;
                        }
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::QuestionToken;
                    }
                }
                (_, b'[') if ch >= 0 => self.one(Kind::OpenBracketToken),
                (_, b']') if ch >= 0 => self.one(Kind::CloseBracketToken),
                (_, b'^') if ch >= 0 => self.two('=', Kind::CaretEqualsToken, Kind::CaretToken),
                (_, b'{') if ch >= 0 => self.one(Kind::OpenBraceToken),
                (_, b'|') if ch >= 0 => {
                    if self.char_at(1) == '|' as i32 && is_conflict_marker_trivia(self.text, self.state.pos) {
                        self.state.pos = self.scan_conflict_marker_trivia(self.state.pos);
                        if self.skip_trivia {
                            continue;
                        }
                        self.state.token = Kind::ConflictMarkerTrivia;
                        return self.state.token;
                    }
                    if self.char_at(1) == '|' as i32 {
                        if self.char_at(2) == '=' as i32 {
                            self.state.pos += 3;
                            self.state.token = Kind::BarBarEqualsToken;
                        } else {
                            self.state.pos += 2;
                            self.state.token = Kind::BarBarToken;
                        }
                    } else if self.char_at(1) == '=' as i32 {
                        self.state.pos += 2;
                        self.state.token = Kind::BarEqualsToken;
                    } else {
                        self.state.pos += 1;
                        self.state.token = Kind::BarToken;
                    }
                }
                (_, b'}') if ch >= 0 => self.one(Kind::CloseBraceToken),
                (_, b'~') if ch >= 0 => self.one(Kind::TildeToken),
                (_, b'@') if ch >= 0 => self.one(Kind::AtToken),
                (_, b'\\') if ch >= 0 => {
                    let cp = self.peek_unicode_escape();
                    if cp >= 0 && chars::is_identifier_start(cp) {
                        let mut value = String::new();
                        let escaped = self.scan_unicode_escape(true);
                        chars::push_rune(&mut value, escaped);
                        value.push_str(&self.scan_identifier_parts());
                        self.state.token_value = value;
                        self.state.token = get_identifier_token(&self.state.token_value);
                    } else {
                        self.scan_invalid_character();
                    }
                }
                (_, b'#') if ch >= 0 => {
                    if self.char_at(1) == '!' as i32 {
                        if self.state.pos == 0 {
                            self.state.pos += 2;
                            loop {
                                let (ch, size) = self.char_and_size();
                                if size == 0 || chars::is_line_break(ch) {
                                    break;
                                }
                                self.state.pos += size;
                            }
                            continue;
                        }
                        self.error_at(diagnostics::X_can_only_be_used_at_the_start_of_a_file, self.state.pos, 2, Vec::new());
                        self.state.pos += 1;
                        self.state.token = Kind::Unknown;
                        return self.state.token;
                    }
                    if self.char_at(1) == '\\' as i32 {
                        self.state.pos += 1;
                        let cp = self.peek_unicode_escape();
                        if cp >= 0 && chars::is_identifier_start(cp) {
                            let mut value = "#".to_string();
                            let escaped = self.scan_unicode_escape(true);
                            chars::push_rune(&mut value, escaped);
                            value.push_str(&self.scan_identifier_parts());
                            self.state.token_value = value;
                            self.state.token = Kind::PrivateIdentifier;
                            return self.state.token;
                        }
                        self.state.pos -= 1;
                    }
                    if !self.scan_identifier(1) {
                        self.error_at(diagnostics::Invalid_character, self.state.pos - 1, 1, Vec::new());
                        self.state.token_value = "#".to_string();
                    }
                    self.state.token = Kind::PrivateIdentifier;
                }
                _ => {
                    if ch < 0 {
                        self.state.token = Kind::EndOfFile;
                        return self.state.token;
                    }
                    if self.scan_identifier(0) {
                        self.state.token = get_identifier_token(&self.state.token_value);
                        return self.state.token;
                    }
                    let (ch, size) = self.char_and_size();
                    if ch == RUNE_ERROR {
                        self.error_at(diagnostics::File_appears_to_be_binary, 0, 0, Vec::new());
                        self.state.pos = self.text.len();
                        self.state.token = Kind::NonTextFileMarkerTrivia;
                        return self.state.token;
                    }
                    if chars::is_white_space_single_line(ch) {
                        self.state.pos += size;
                        if ch == 0x85 || self.skip_trivia {
                            continue;
                        }
                        loop {
                            let (ch, size) = self.char_and_size();
                            if !chars::is_white_space_single_line(ch) {
                                break;
                            }
                            self.state.pos += size;
                        }
                        self.state.token = Kind::WhitespaceTrivia;
                        return self.state.token;
                    }
                    if chars::is_line_break(ch) {
                        self.state.token_flags |= TokenFlags::PrecedingLineBreak;
                        self.state.pos += size;
                        continue;
                    }
                    self.scan_invalid_character();
                }
            }
            return self.state.token;
        }
    }

    fn one(&mut self, kind: Kind) {
        self.state.pos += 1;
        self.state.token = kind;
    }

    fn two(&mut self, next: char, double: Kind, single: Kind) {
        if self.char_at(1) == next as i32 {
            self.state.pos += 2;
            self.state.token = double;
        } else {
            self.state.pos += 1;
            self.state.token = single;
        }
    }

    pub fn re_scan_less_than_token(&mut self) -> Kind {
        if self.state.token == Kind::LessThanLessThanToken {
            self.state.pos = self.state.token_start + 1;
            self.state.token = Kind::LessThanToken;
        }
        self.state.token
    }

    pub fn re_scan_greater_than_token(&mut self) -> Kind {
        if self.state.token == Kind::GreaterThanToken {
            self.state.pos = self.state.token_start + 1;
            if self.char() == '>' as i32 {
                if self.char_at(1) == '>' as i32 {
                    if self.char_at(2) == '=' as i32 {
                        self.state.pos += 3;
                        self.state.token = Kind::GreaterThanGreaterThanGreaterThanEqualsToken;
                    } else {
                        self.state.pos += 2;
                        self.state.token = Kind::GreaterThanGreaterThanGreaterThanToken;
                    }
                } else if self.char_at(1) == '=' as i32 {
                    self.state.pos += 2;
                    self.state.token = Kind::GreaterThanGreaterThanEqualsToken;
                } else {
                    self.state.pos += 1;
                    self.state.token = Kind::GreaterThanGreaterThanToken;
                }
            } else if self.char() == '=' as i32 {
                self.state.pos += 1;
                self.state.token = Kind::GreaterThanEqualsToken;
            }
        }
        self.state.token
    }

    pub fn re_scan_template_token(&mut self, is_tagged_template: bool) -> Kind {
        self.state.pos = self.state.token_start;
        self.state.token = self.scan_template_and_set_token_value(!is_tagged_template);
        self.state.token
    }

    pub fn re_scan_asterisk_equals_token(&mut self) -> Kind {
        self.state.pos = self.state.token_start + 1;
        self.state.token = Kind::EqualsToken;
        self.state.token
    }

    /// `ReScanSlashToken()` as the parser calls it: without reporting the
    /// regular expression's own errors, which the checker validates.
    pub fn re_scan_slash_token(&mut self) -> Kind {
        if self.state.token != Kind::SlashToken && self.state.token != Kind::SlashEqualsToken {
            return self.state.token;
        }
        let bytes = self.text.as_bytes();
        let start_of_body = self.state.token_start + 1;
        let mut p = start_of_body;
        let mut in_escape = false;
        let mut in_character_class = false;
        loop {
            if p >= self.end {
                self.state.token_flags |= TokenFlags::Unterminated;
                break;
            }
            let ch = bytes[p] as i32;
            if chars::is_line_break(ch) {
                self.state.token_flags |= TokenFlags::Unterminated;
                break;
            } else if in_escape {
                in_escape = false;
            } else if ch == '/' as i32 && !in_character_class {
                break;
            } else if ch == '[' as i32 {
                in_character_class = true;
            } else if ch == '\\' as i32 {
                in_escape = true;
            } else if ch == ']' as i32 {
                in_character_class = false;
            }
            p += 1;
        }
        let end_of_body = p;
        if self.state.token_flags.has(TokenFlags::Unterminated) {
            p = start_of_body;
            in_escape = false;
            let mut character_class_depth = 0;
            let mut in_decimal_quantifier = false;
            let mut group_depth = 0;
            while p < end_of_body {
                let ch = bytes[p];
                if in_escape {
                    in_escape = false;
                } else if ch == b'\\' {
                    in_escape = true;
                } else if ch == b'[' {
                    character_class_depth += 1;
                } else if ch == b']' && character_class_depth != 0 {
                    character_class_depth -= 1;
                } else if character_class_depth == 0 {
                    if ch == b'{' {
                        in_decimal_quantifier = true;
                    } else if ch == b'}' && in_decimal_quantifier {
                        in_decimal_quantifier = false;
                    } else if !in_decimal_quantifier {
                        if ch == b'(' {
                            group_depth += 1;
                        } else if ch == b')' && group_depth != 0 {
                            group_depth -= 1;
                        } else if ch == b')' || ch == b']' || ch == b'}' {
                            break;
                        }
                    }
                }
                p += 1;
            }
            while p > start_of_body {
                let (ch, size) = decode_last_rune(self.text, p);
                if chars::is_white_space_like(ch) || ch == ';' as i32 {
                    p -= size;
                } else {
                    break;
                }
            }
            let token_start = self.state.token_start;
            self.error_at(diagnostics::Unterminated_regular_expression_literal, token_start, p - token_start, Vec::new());
        } else {
            p += 1;
            while p < self.end {
                let (ch, size) = decode_rune(self.text, p, self.end);
                if ch == RUNE_ERROR || !chars::is_identifier_part(ch) {
                    break;
                }
                p += size;
            }
        }
        self.state.pos = p;
        self.state.token_value = self.text[self.state.token_start..self.state.pos].to_string();
        self.state.token = Kind::RegularExpressionLiteral;
        self.state.token
    }

    pub fn re_scan_jsx_token(&mut self, allow_multiline_jsx_text: bool) -> Kind {
        self.state.pos = self.state.full_start_pos;
        self.state.token_start = self.state.full_start_pos;
        self.state.token = self.scan_jsx_token_ex(allow_multiline_jsx_text);
        self.state.token
    }


    pub fn re_scan_question_token(&mut self) -> Kind {
        self.state.pos = self.state.token_start + 1;
        self.state.token = Kind::QuestionToken;
        self.state.token
    }

    pub fn scan_jsx_token(&mut self) -> Kind {
        self.scan_jsx_token_ex(true)
    }

    pub fn scan_jsx_token_ex(&mut self, allow_multiline_jsx_text: bool) -> Kind {
        self.state.full_start_pos = self.state.pos;
        self.state.token_start = self.state.pos;
        let ch = self.char();
        if ch < 0 {
            self.state.token = Kind::EndOfFile;
        } else if ch == '<' as i32 {
            if self.char_at(1) == '/' as i32 {
                self.state.pos += 2;
                self.state.token = Kind::LessThanSlashToken;
            } else {
                self.state.pos += 1;
                self.state.token = Kind::LessThanToken;
            }
        } else if ch == '{' as i32 {
            self.state.pos += 1;
            self.state.token = Kind::OpenBraceToken;
        } else {
            let mut first_non_whitespace: isize = 0;
            loop {
                let (ch, size) = self.char_and_size();
                if size == 0 || ch == '{' as i32 {
                    break;
                }
                if ch == '<' as i32 {
                    if is_conflict_marker_trivia(self.text, self.state.pos) {
                        self.state.pos = self.scan_conflict_marker_trivia(self.state.pos);
                        self.state.token = Kind::ConflictMarkerTrivia;
                        return self.state.token;
                    }
                    break;
                }
                if ch == '>' as i32 {
                    self.error_at(diagnostics::Unexpected_token_Did_you_mean_or_gt, self.state.pos, 1, Vec::new());
                } else if ch == '}' as i32 {
                    self.error_at(diagnostics::Unexpected_token_Did_you_mean_or_rbrace, self.state.pos, 1, Vec::new());
                }
                if chars::is_line_break(ch) && first_non_whitespace == 0 {
                    first_non_whitespace = -1;
                } else if !allow_multiline_jsx_text && chars::is_line_break(ch) && first_non_whitespace > 0 {
                    break;
                } else if !chars::is_white_space_like(ch) {
                    first_non_whitespace = self.state.pos as isize;
                }
                self.state.pos += size;
            }
            self.state.token_value = self.text[self.state.full_start_pos..self.state.pos].to_string();
            self.state.token = if first_non_whitespace == -1 { Kind::JsxTextAllWhiteSpaces } else { Kind::JsxText };
        }
        self.state.token
    }

    pub fn scan_jsx_identifier(&mut self) -> Kind {
        if token_is_identifier_or_keyword(self.state.token) {
            loop {
                let ch = self.char();
                if ch < 0 {
                    break;
                }
                if ch == '-' as i32 {
                    self.state.token_value.push('-');
                    self.state.pos += 1;
                    continue;
                }
                let old_pos = self.state.pos;
                let parts = self.scan_identifier_parts();
                self.state.token_value.push_str(&parts);
                if self.state.pos == old_pos {
                    break;
                }
            }
            self.state.token = get_identifier_token(&self.state.token_value);
        }
        self.state.token
    }

    pub fn scan_jsx_attribute_value(&mut self) -> Kind {
        self.state.full_start_pos = self.state.pos;
        loop {
            let (ch, size) = self.char_and_size();
            if size == 0 || !chars::is_white_space_like(ch) {
                break;
            }
            self.state.pos += size;
        }
        self.state.token_start = self.state.pos;
        let ch = self.char();
        if ch == '"' as i32 || ch == '\'' as i32 {
            self.state.token_value = self.scan_string(true);
            self.state.token = Kind::StringLiteral;
            return self.state.token;
        }
        self.scan()
    }


    fn scan_identifier(&mut self, prefix_length: usize) -> bool {
        let start = self.state.pos;
        self.state.pos += prefix_length;
        let ch = self.char();
        if chars::is_ascii_letter(ch) || ch == '_' as i32 || ch == '$' as i32 {
            self.state.pos += 1;
            self.scan_ascii_while(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$');
            let ch = self.char();
            if ch < 0x80 && ch != '\\' as i32 {
                self.state.token_value = self.text[start..self.state.pos].to_string();
                return true;
            }
            self.state.pos = start + prefix_length;
        }
        let (mut ch, mut size) = self.char_and_size();
        if chars::is_identifier_start(ch) {
            loop {
                self.state.pos += size;
                (ch, size) = self.char_and_size();
                if !chars::is_identifier_part(ch) {
                    break;
                }
            }
            let mut value = self.text[start..self.state.pos].to_string();
            if ch == '\\' as i32 {
                value.push_str(&self.scan_identifier_parts());
            }
            self.state.token_value = value;
            return true;
        }
        false
    }

    fn scan_identifier_parts(&mut self) -> String {
        let mut out = String::new();
        let mut start = self.state.pos;
        loop {
            let (ch, size) = self.char_and_size();
            if chars::is_identifier_part(ch) {
                self.state.pos += size;
                continue;
            }
            if ch == '\\' as i32 {
                let escaped = self.peek_unicode_escape();
                if escaped >= 0 && chars::is_identifier_part(escaped) {
                    out.push_str(&self.text[start..self.state.pos]);
                    let rune = self.scan_unicode_escape(true);
                    chars::push_rune(&mut out, rune);
                    start = self.state.pos;
                    continue;
                }
            }
            break;
        }
        out.push_str(&self.text[start..self.state.pos]);
        out
    }

    fn scan_string(&mut self, jsx_attribute_string: bool) -> String {
        let quote = self.char();
        if quote == '\'' as i32 {
            self.state.token_flags |= TokenFlags::SingleQuote;
        }
        self.state.pos += 1;
        let rest = &self.text[self.state.pos..];
        if let Some(len) = rest.bytes().position(|b| b as i32 == quote) {
            if len == 0 {
                self.state.pos += 1;
                return String::new();
            }
            let s = &rest[..len];
            if jsx_attribute_string || !s.bytes().any(|b| b == b'\\' || b == b'\r' || b == b'\n') {
                self.state.pos += len + 1;
                return s.to_string();
            }
        }
        let mut out = String::new();
        let mut start = self.state.pos;
        loop {
            let ch = self.char();
            if ch < 0 {
                out.push_str(&self.text[start..self.state.pos]);
                self.state.token_flags |= TokenFlags::Unterminated;
                self.error(diagnostics::Unterminated_string_literal);
                break;
            }
            if ch == quote {
                out.push_str(&self.text[start..self.state.pos]);
                self.state.pos += 1;
                break;
            }
            if ch == '\\' as i32 && !jsx_attribute_string {
                out.push_str(&self.text[start..self.state.pos]);
                let escaped = self.scan_escape_sequence(ESCAPE_STRING | ESCAPE_REPORT_ERRORS);
                out.push_str(&escaped);
                start = self.state.pos;
                continue;
            }
            if (ch == '\n' as i32 || ch == '\r' as i32) && !jsx_attribute_string {
                out.push_str(&self.text[start..self.state.pos]);
                self.state.token_flags |= TokenFlags::Unterminated;
                self.error(diagnostics::Unterminated_string_literal);
                break;
            }
            self.state.pos += 1;
        }
        out
    }

    fn scan_template_and_set_token_value(&mut self, should_emit_invalid_escape_error: bool) -> Kind {
        let started_with_backtick = self.char() == '`' as i32;
        self.state.pos += 1;
        let mut start = self.state.pos;
        let mut value = String::new();
        let token;
        loop {
            self.scan_ascii_while(|b| b != b'`' && b != b'$' && b != b'\\' && b != b'\r');
            let ch = self.char();
            if ch < 0 || ch == '`' as i32 {
                value.push_str(&self.text[start..self.state.pos]);
                if ch == '`' as i32 {
                    self.state.pos += 1;
                } else {
                    self.state.token_flags |= TokenFlags::Unterminated;
                    self.error(diagnostics::Unterminated_template_literal);
                }
                token = if started_with_backtick { Kind::NoSubstitutionTemplateLiteral } else { Kind::TemplateTail };
                break;
            }
            if ch == '$' as i32 && self.char_at(1) == '{' as i32 {
                value.push_str(&self.text[start..self.state.pos]);
                self.state.pos += 2;
                token = if started_with_backtick { Kind::TemplateHead } else { Kind::TemplateMiddle };
                break;
            }
            if ch == '\\' as i32 {
                value.push_str(&self.text[start..self.state.pos]);
                let flags = ESCAPE_STRING | if should_emit_invalid_escape_error { ESCAPE_REPORT_ERRORS } else { 0 };
                let escaped = self.scan_escape_sequence(flags);
                value.push_str(&escaped);
                start = self.state.pos;
                continue;
            }
            if ch == '\r' as i32 {
                value.push_str(&self.text[start..self.state.pos]);
                self.state.pos += 1;
                if self.char() == '\n' as i32 {
                    self.state.pos += 1;
                }
                value.push('\n');
                start = self.state.pos;
                continue;
            }
            self.state.pos += 1;
        }
        self.state.token_value = value;
        token
    }

    fn scan_escape_sequence(&mut self, flags: u32) -> String {
        let start = self.state.pos;
        self.state.pos += 1;
        let ch = self.char();
        if ch < 0 {
            self.error(diagnostics::Unexpected_end_of_text);
            return String::new();
        }
        self.state.pos += 1;
        let report_invalid = flags & ESCAPE_REPORT_INVALID_ESCAPE_ERRORS != 0;
        match ch as u8 {
            b'0'..=b'7' if ch < 0x80 => {
                let c = ch as u8;
                if c == b'0' && !chars::is_digit(self.char()) {
                    return "\0".to_string();
                }
                if c <= b'3' && chars::is_octal_digit(self.char()) {
                    self.state.pos += 1;
                }
                if chars::is_octal_digit(self.char()) {
                    self.state.pos += 1;
                }
                self.state.token_flags |= TokenFlags::ContainsInvalidEscape;
                if report_invalid {
                    let code = i32::from_str_radix(&self.text[start + 1..self.state.pos], 8).unwrap_or(0);
                    let arg = format!("\\x{code:02x}");
                    self.error_at(diagnostics::Octal_escape_sequences_are_not_allowed_Use_the_syntax_0, start, self.state.pos - start, vec![arg]);
                    let mut out = String::new();
                    chars::push_rune(&mut out, code);
                    return out;
                }
                self.text[start..self.state.pos].to_string()
            }
            b'8' | b'9' if ch < 0x80 => {
                self.state.token_flags |= TokenFlags::ContainsInvalidEscape;
                if report_invalid {
                    let text = self.text[start..self.state.pos].to_string();
                    self.error_at(diagnostics::Escape_sequence_0_is_not_allowed, start, self.state.pos - start, vec![text]);
                    return (ch as u8 as char).to_string();
                }
                self.text[start..self.state.pos].to_string()
            }
            b'b' => "\u{8}".to_string(),
            b't' => "\t".to_string(),
            b'n' => "\n".to_string(),
            b'v' => "\u{b}".to_string(),
            b'f' => "\u{c}".to_string(),
            b'r' => "\r".to_string(),
            b'\'' => "'".to_string(),
            b'"' => "\"".to_string(),
            b'u' => {
                let extended = self.char() == '{' as i32;
                self.state.pos -= 2;
                let code_point = self.scan_unicode_escape(report_invalid);
                if extended {
                    if flags & ESCAPE_ALLOW_EXTENDED_UNICODE_ESCAPE == 0 {
                        self.state.token_flags |= TokenFlags::ContainsInvalidEscape;
                    }
                    if code_point < 0 {
                        return self.text[start..self.state.pos].to_string();
                    }
                    if chars::is_high_surrogate(code_point)
                        && let Some(combined) = self.scan_low_surrogate_escape(code_point)
                    {
                        let mut out = String::new();
                        chars::push_rune(&mut out, combined);
                        return out;
                    }
                    let mut out = String::new();
                    chars::push_rune(&mut out, code_point);
                    return out;
                }
                if code_point < 0 {
                    return self.text[start..self.state.pos].to_string();
                }
                if chars::is_high_surrogate(code_point)
                    && let Some(combined) = self.scan_low_surrogate_escape(code_point)
                {
                    let mut out = String::new();
                    chars::push_rune(&mut out, combined);
                    return out;
                }
                let mut out = String::new();
                chars::push_rune(&mut out, code_point);
                out
            }
            b'x' => {
                while self.state.pos < start + 4 {
                    if !chars::is_hex_digit(self.char()) {
                        self.state.token_flags |= TokenFlags::ContainsInvalidEscape;
                        if report_invalid {
                            self.error(diagnostics::Hexadecimal_digit_expected);
                        }
                        return self.text[start..self.state.pos].to_string();
                    }
                    self.state.pos += 1;
                }
                self.state.token_flags |= TokenFlags::HexEscape;
                let value = i32::from_str_radix(&self.text[start + 2..self.state.pos], 16).unwrap_or(0);
                let mut out = String::new();
                chars::push_rune(&mut out, value);
                out
            }
            b'\r' => {
                if self.char() == '\n' as i32 {
                    self.state.pos += 1;
                }
                String::new()
            }
            b'\n' => String::new(),
            _ => {
                let mut ch = ch;
                if ch >= 0x80 {
                    self.state.pos -= 1;
                    let (decoded, size) = self.char_and_size();
                    ch = decoded;
                    self.state.pos += size;
                }
                if ch == 0x2028 || ch == 0x2029 {
                    return String::new();
                }
                let mut out = String::new();
                chars::push_rune(&mut out, ch);
                out
            }
        }
    }

    fn scan_unicode_escape(&mut self, should_emit_invalid_escape_error: bool) -> i32 {
        self.state.pos += 2;
        let start = self.state.pos;
        let extended = self.char() == '{' as i32;
        let hex_digits = if extended {
            self.state.pos += 1;
            self.scan_hex_digits(1, true, false)
        } else {
            self.state.token_flags |= TokenFlags::UnicodeEscape;
            self.scan_hex_digits(4, false, false)
        };
        if hex_digits.is_empty() {
            self.state.token_flags |= TokenFlags::ContainsInvalidEscape;
            if should_emit_invalid_escape_error {
                self.error(diagnostics::Hexadecimal_digit_expected);
            }
            return -1;
        }
        let hex_value = i64::from_str_radix(&hex_digits, 16).unwrap_or(i64::MAX);
        if extended {
            let mut invalid = false;
            if hex_value > 0x10FFFF {
                if should_emit_invalid_escape_error {
                    self.error_at(
                        diagnostics::An_extended_Unicode_escape_value_must_be_between_0x0_and_0x10FFFF_inclusive,
                        start + 1,
                        self.state.pos - start - 1,
                        Vec::new(),
                    );
                }
                invalid = true;
            }
            if self.state.pos >= self.end {
                if should_emit_invalid_escape_error {
                    self.error(diagnostics::Unexpected_end_of_text);
                }
                invalid = true;
            } else if self.char() == '}' as i32 {
                self.state.pos += 1;
            } else {
                if should_emit_invalid_escape_error {
                    self.error(diagnostics::Unterminated_Unicode_escape_sequence);
                }
                invalid = true;
            }
            if invalid {
                self.state.token_flags |= TokenFlags::ContainsInvalidEscape;
                return -1;
            }
            self.state.token_flags |= TokenFlags::ExtendedUnicodeEscape;
        }
        hex_value as i32
    }

    fn scan_low_surrogate_escape(&mut self, high: i32) -> Option<i32> {
        if self.char() != '\\' as i32 || self.char_at(1) != 'u' as i32 {
            return None;
        }
        let saved_pos = self.state.pos;
        let saved_flags = self.state.token_flags;
        let low = self.scan_unicode_escape(false);
        if chars::is_low_surrogate(low) {
            return Some(chars::surrogate_pair_to_code_point(high, low));
        }
        self.state.pos = saved_pos;
        self.state.token_flags = saved_flags;
        None
    }

    fn peek_unicode_escape(&mut self) -> i32 {
        if self.char_at(1) == 'u' as i32 {
            let save_pos = self.state.pos;
            let save_flags = self.state.token_flags;
            let code_point = self.scan_unicode_escape(false);
            self.state.pos = save_pos;
            self.state.token_flags = save_flags;
            return code_point;
        }
        -1
    }

    fn scan_number(&mut self) -> Kind {
        let mut start = self.state.pos;
        let fixed_part;
        if self.char() == '0' as i32 {
            self.state.pos += 1;
            if self.char() == '_' as i32 {
                self.state.token_flags |= TokenFlags::ContainsSeparator | TokenFlags::ContainsInvalidSeparator;
                self.error_at(diagnostics::Numeric_separators_are_not_allowed_here, self.state.pos, 1, Vec::new());
                self.state.pos = start;
                fixed_part = self.scan_number_fragment();
            } else {
                let (digits, is_octal) = self.scan_digits();
                if digits.is_empty() {
                    fixed_part = "0".to_string();
                } else if !is_octal {
                    self.state.token_flags |= TokenFlags::ContainsLeadingZero;
                    fixed_part = digits;
                } else {
                    let value = u64::from_str_radix(&digits, 8).unwrap_or(0);
                    self.state.token_value = value.to_string();
                    self.state.token_flags |= TokenFlags::Octal;
                    let with_minus = self.state.token == Kind::MinusToken;
                    let literal = format!("{}0o{value:o}", if with_minus { "-" } else { "" });
                    if with_minus {
                        start -= 1;
                    }
                    self.error_at(diagnostics::Octal_literals_are_not_allowed_Use_the_syntax_0, start, self.state.pos - start, vec![literal]);
                    return Kind::NumericLiteral;
                }
            }
        } else {
            fixed_part = self.scan_number_fragment();
        }
        let fixed_part_end = self.state.pos;
        let mut fractional_part = String::new();
        let mut exponent_preamble = "";
        let mut exponent_part = String::new();
        if self.char() == '.' as i32 {
            self.state.pos += 1;
            fractional_part = self.scan_number_fragment();
        }
        let mut end = self.state.pos;
        if self.char() == 'E' as i32 || self.char() == 'e' as i32 {
            self.state.pos += 1;
            self.state.token_flags |= TokenFlags::Scientific;
            if self.char() == '+' as i32 || self.char() == '-' as i32 {
                self.state.pos += 1;
            }
            let start_numeric_part = self.state.pos;
            exponent_part = self.scan_number_fragment();
            if exponent_part.is_empty() {
                self.error(diagnostics::Digit_expected);
            } else {
                exponent_preamble = &self.text[end..start_numeric_part];
                end = self.state.pos;
            }
        }
        if self.state.token_flags.has(TokenFlags::ContainsSeparator) {
            let mut value = fixed_part;
            if !fractional_part.is_empty() {
                value.push('.');
                value.push_str(&fractional_part);
            }
            if !exponent_part.is_empty() {
                value.push_str(exponent_preamble);
                value.push_str(&exponent_part);
            }
            self.state.token_value = value;
        } else {
            self.state.token_value = self.text[start..end].to_string();
        }
        if self.state.token_flags.has(TokenFlags::ContainsLeadingZero) {
            self.error_at(diagnostics::Decimals_with_leading_zeros_are_not_allowed, start, self.state.pos - start, Vec::new());
            self.state.token_value = js_number_string(&self.state.token_value);
            return Kind::NumericLiteral;
        }
        let result = if fixed_part_end == self.state.pos {
            self.scan_big_int_suffix()
        } else {
            self.state.token_value = js_number_string(&self.state.token_value);
            Kind::NumericLiteral
        };
        let (ch, _) = self.char_and_size();
        if chars::is_identifier_start(ch) {
            let id_start = self.state.pos;
            let id = self.scan_identifier_parts();
            if result != Kind::BigIntLiteral && id.len() == 1 && self.text.as_bytes()[id_start] == b'n' {
                if self.state.token_flags.has(TokenFlags::Scientific) {
                    self.error_at(diagnostics::A_bigint_literal_cannot_use_exponential_notation, start, self.state.pos - start, Vec::new());
                    return result;
                }
                if fixed_part_end < id_start {
                    self.error_at(diagnostics::A_bigint_literal_must_be_an_integer, start, self.state.pos - start, Vec::new());
                    return result;
                }
            }
            self.error_at(
                diagnostics::An_identifier_or_keyword_cannot_immediately_follow_a_numeric_literal,
                id_start,
                self.state.pos - id_start,
                Vec::new(),
            );
            self.state.pos = id_start;
        }
        result
    }

    fn scan_number_fragment(&mut self) -> String {
        let mut start = self.state.pos;
        let mut allow_separator = false;
        let mut is_previous_token_separator = false;
        let mut result = String::new();
        loop {
            let before = self.state.pos;
            self.scan_ascii_while(|b| b.is_ascii_digit());
            if self.state.pos > before {
                allow_separator = true;
                is_previous_token_separator = false;
            }
            if self.char() == '_' as i32 {
                self.state.token_flags |= TokenFlags::ContainsSeparator;
                if allow_separator {
                    allow_separator = false;
                    is_previous_token_separator = true;
                    result.push_str(&self.text[start..self.state.pos]);
                } else {
                    self.state.token_flags |= TokenFlags::ContainsInvalidSeparator;
                    if is_previous_token_separator {
                        self.error_at(diagnostics::Multiple_consecutive_numeric_separators_are_not_permitted, self.state.pos, 1, Vec::new());
                    } else {
                        self.error_at(diagnostics::Numeric_separators_are_not_allowed_here, self.state.pos, 1, Vec::new());
                    }
                }
                self.state.pos += 1;
                start = self.state.pos;
                continue;
            }
            break;
        }
        if is_previous_token_separator {
            self.state.token_flags |= TokenFlags::ContainsInvalidSeparator;
            self.error_at(diagnostics::Numeric_separators_are_not_allowed_here, self.state.pos - 1, 1, Vec::new());
        }
        if result.is_empty() {
            return self.text[start..self.state.pos].to_string();
        }
        result.push_str(&self.text[start..self.state.pos]);
        result
    }

    fn scan_digits(&mut self) -> (String, bool) {
        let start = self.state.pos;
        let mut is_octal = true;
        while chars::is_digit(self.char()) {
            if !chars::is_octal_digit(self.char()) {
                is_octal = false;
            }
            self.state.pos += 1;
        }
        (self.text[start..self.state.pos].to_string(), is_octal)
    }

    fn scan_hex_digits(&mut self, min_count: usize, scan_as_many_as_possible: bool, can_have_separators: bool) -> String {
        let mut digit_count = 0;
        let start = self.state.pos;
        let mut allow_separator = false;
        let mut is_previous_token_separator = false;
        while digit_count < min_count || scan_as_many_as_possible {
            let ch = self.char();
            if chars::is_hex_digit(ch) {
                allow_separator = can_have_separators;
                is_previous_token_separator = false;
                digit_count += 1;
            } else if can_have_separators && ch == '_' as i32 {
                self.state.token_flags |= TokenFlags::ContainsSeparator;
                if allow_separator {
                    allow_separator = false;
                    is_previous_token_separator = true;
                } else if is_previous_token_separator {
                    self.error_at(diagnostics::Multiple_consecutive_numeric_separators_are_not_permitted, self.state.pos, 1, Vec::new());
                } else {
                    self.error_at(diagnostics::Numeric_separators_are_not_allowed_here, self.state.pos, 1, Vec::new());
                }
            } else {
                break;
            }
            self.state.pos += 1;
        }
        if is_previous_token_separator {
            self.error_at(diagnostics::Numeric_separators_are_not_allowed_here, self.state.pos - 1, 1, Vec::new());
        }
        if digit_count < min_count {
            return String::new();
        }
        self.text[start..self.state.pos].replace('_', "").to_ascii_lowercase()
    }

    fn scan_binary_or_octal_digits(&mut self, base: i32) -> String {
        let mut out = String::new();
        let mut allow_separator = false;
        let mut is_previous_token_separator = false;
        loop {
            let ch = self.char();
            if chars::is_digit(ch) && (ch - '0' as i32) < base {
                out.push(ch as u8 as char);
                allow_separator = true;
                is_previous_token_separator = false;
            } else if ch == '_' as i32 {
                self.state.token_flags |= TokenFlags::ContainsSeparator;
                if allow_separator {
                    allow_separator = false;
                    is_previous_token_separator = true;
                } else if is_previous_token_separator {
                    self.error_at(diagnostics::Multiple_consecutive_numeric_separators_are_not_permitted, self.state.pos, 1, Vec::new());
                } else {
                    self.error_at(diagnostics::Numeric_separators_are_not_allowed_here, self.state.pos, 1, Vec::new());
                }
            } else {
                break;
            }
            self.state.pos += 1;
        }
        if is_previous_token_separator {
            self.error_at(diagnostics::Numeric_separators_are_not_allowed_here, self.state.pos - 1, 1, Vec::new());
        }
        out
    }

    fn scan_big_int_suffix(&mut self) -> Kind {
        if self.char() == 'n' as i32 {
            self.state.token_value.push('n');
            self.state.pos += 1;
            return Kind::BigIntLiteral;
        }
        self.state.token_value = js_number_string(&self.state.token_value);
        Kind::NumericLiteral
    }

    fn scan_invalid_character(&mut self) {
        let (_, size) = self.char_and_size();
        self.error_at(diagnostics::Invalid_character, self.state.pos, size, Vec::new());
        self.state.pos += size;
        self.state.token = Kind::Unknown;
    }

    fn scan_conflict_marker_trivia(&mut self, pos: usize) -> usize {
        self.error_at(diagnostics::Merge_conflict_marker_encountered, pos, MERGE_CONFLICT_MARKER_LENGTH, Vec::new());
        scan_conflict_marker_trivia(self.text, pos)
    }
}

const ESCAPE_STRING: u32 = 1 << 0;
const ESCAPE_REPORT_ERRORS: u32 = 1 << 1;
const ESCAPE_REGULAR_EXPRESSION: u32 = 1 << 2;
const ESCAPE_ANY_UNICODE_MODE: u32 = 1 << 4;
const ESCAPE_REPORT_INVALID_ESCAPE_ERRORS: u32 = ESCAPE_REGULAR_EXPRESSION | ESCAPE_REPORT_ERRORS;
const ESCAPE_ALLOW_EXTENDED_UNICODE_ESCAPE: u32 = ESCAPE_STRING | ESCAPE_ANY_UNICODE_MODE;
const MERGE_CONFLICT_MARKER_LENGTH: usize = 7;

/// `utf8.DecodeRuneInString`: `(RuneError, 0)` at the end of the text.
pub fn decode_rune(text: &str, pos: usize, end: usize) -> (i32, usize) {
    if pos >= end {
        return (RUNE_ERROR, 0);
    }
    let b = text.as_bytes()[pos];
    if b < 0x80 {
        return (b as i32, 1);
    }
    match text[pos..].chars().next() {
        Some(ch) => (ch as i32, ch.len_utf8()),
        None => (RUNE_ERROR, 1),
    }
}

fn decode_last_rune(text: &str, end: usize) -> (i32, usize) {
    match text[..end].chars().next_back() {
        Some(ch) => (ch as i32, ch.len_utf8()),
        None => (RUNE_ERROR, 0),
    }
}

pub fn token_is_identifier_or_keyword(token: Kind) -> bool {
    token >= Kind::Identifier
}

pub fn get_identifier_token(text: &str) -> Kind {
    let bytes = text.as_bytes();
    if (2..=12).contains(&bytes.len()) && bytes[0].is_ascii_lowercase() {
        if let Some(keyword) = keyword_kind(text) {
            return keyword;
        }
    }
    Kind::Identifier
}

pub fn keyword_kind(text: &str) -> Option<Kind> {
    TEXT_TO_KEYWORD.iter().find(|(word, _)| *word == text).map(|(_, kind)| *kind)
}

pub fn token_to_string(token: Kind) -> &'static str {
    TEXT_TO_KEYWORD
        .iter()
        .chain(TEXT_TO_PUNCTUATION.iter())
        .find(|(_, kind)| *kind == token)
        .map_or("", |(text, _)| text)
}


pub fn is_conflict_marker_trivia(text: &str, pos: usize) -> bool {
    let bytes = text.as_bytes();
    if pos + 1 >= bytes.len() || bytes[pos + 1] != bytes[pos] {
        return false;
    }
    let mut at_line_start = pos == 0 || chars::is_line_break(bytes[pos - 1] as i32);
    if !at_line_start && pos >= 2 {
        let (prev, _) = decode_last_rune(text, pos - 2);
        at_line_start = chars::is_line_break(prev);
    }
    if at_line_start {
        let ch = bytes[pos];
        if pos + MERGE_CONFLICT_MARKER_LENGTH < bytes.len() {
            if (0..MERGE_CONFLICT_MARKER_LENGTH).any(|i| bytes[pos + i] != ch) {
                return false;
            }
            return ch == b'=' || bytes[pos + MERGE_CONFLICT_MARKER_LENGTH] == b' ';
        }
    }
    false
}

fn scan_conflict_marker_trivia(text: &str, mut pos: usize) -> usize {
    let bytes = text.as_bytes();
    let length = bytes.len();
    let (mut ch, mut size) = decode_rune(text, pos, length);
    if ch == '<' as i32 || ch == '>' as i32 {
        while pos < length && !chars::is_line_break(ch) {
            pos += size;
            (ch, size) = decode_rune(text, pos, length);
        }
    } else {
        while pos < length {
            let current = bytes[pos];
            if (current == b'=' || current == b'>') && current as i32 != ch && is_conflict_marker_trivia(text, pos) {
                break;
            }
            pos += 1;
        }
    }
    pos
}

/// `jsnum.FromString(s).String()`: the literal's value as JavaScript prints it.
fn js_number_string(text: &str) -> String {
    let value = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u128::from_str_radix(hex, 16).map_or(f64::INFINITY, |v| v as f64)
    } else if let Some(bin) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        u128::from_str_radix(bin, 2).map_or(f64::INFINITY, |v| v as f64)
    } else if let Some(oct) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
        u128::from_str_radix(oct, 8).map_or(f64::INFINITY, |v| v as f64)
    } else {
        match text.parse::<f64>() {
            Ok(value) => value,
            Err(_) => return text.to_string(),
        }
    };
    format_js_number(value)
}

fn format_js_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let k = digits.len() as i32;
    let n = exponent + 1;
    let sign = if value < 0.0 { "-" } else { "" };
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let e = if e >= 0 { format!("+{e}") } else { e.to_string() };
        if k == 1 { format!("{digits}e{e}") } else { format!("{}.{}e{e}", &digits[..1], &digits[1..]) }
    };
    format!("{sign}{body}")
}

const TEXT_TO_KEYWORD: &[(&str, Kind)] = &[
    ("abstract", Kind::AbstractKeyword),
    ("accessor", Kind::AccessorKeyword),
    ("any", Kind::AnyKeyword),
    ("as", Kind::AsKeyword),
    ("asserts", Kind::AssertsKeyword),
    ("assert", Kind::AssertKeyword),
    ("bigint", Kind::BigIntKeyword),
    ("boolean", Kind::BooleanKeyword),
    ("break", Kind::BreakKeyword),
    ("case", Kind::CaseKeyword),
    ("catch", Kind::CatchKeyword),
    ("class", Kind::ClassKeyword),
    ("continue", Kind::ContinueKeyword),
    ("const", Kind::ConstKeyword),
    ("constructor", Kind::ConstructorKeyword),
    ("debugger", Kind::DebuggerKeyword),
    ("declare", Kind::DeclareKeyword),
    ("default", Kind::DefaultKeyword),
    ("defer", Kind::DeferKeyword),
    ("delete", Kind::DeleteKeyword),
    ("do", Kind::DoKeyword),
    ("else", Kind::ElseKeyword),
    ("enum", Kind::EnumKeyword),
    ("export", Kind::ExportKeyword),
    ("extends", Kind::ExtendsKeyword),
    ("false", Kind::FalseKeyword),
    ("finally", Kind::FinallyKeyword),
    ("for", Kind::ForKeyword),
    ("from", Kind::FromKeyword),
    ("function", Kind::FunctionKeyword),
    ("get", Kind::GetKeyword),
    ("if", Kind::IfKeyword),
    ("immediate", Kind::ImmediateKeyword),
    ("implements", Kind::ImplementsKeyword),
    ("import", Kind::ImportKeyword),
    ("in", Kind::InKeyword),
    ("infer", Kind::InferKeyword),
    ("instanceof", Kind::InstanceOfKeyword),
    ("interface", Kind::InterfaceKeyword),
    ("intrinsic", Kind::IntrinsicKeyword),
    ("is", Kind::IsKeyword),
    ("keyof", Kind::KeyOfKeyword),
    ("let", Kind::LetKeyword),
    ("module", Kind::ModuleKeyword),
    ("namespace", Kind::NamespaceKeyword),
    ("never", Kind::NeverKeyword),
    ("new", Kind::NewKeyword),
    ("null", Kind::NullKeyword),
    ("number", Kind::NumberKeyword),
    ("object", Kind::ObjectKeyword),
    ("package", Kind::PackageKeyword),
    ("private", Kind::PrivateKeyword),
    ("protected", Kind::ProtectedKeyword),
    ("public", Kind::PublicKeyword),
    ("override", Kind::OverrideKeyword),
    ("out", Kind::OutKeyword),
    ("readonly", Kind::ReadonlyKeyword),
    ("require", Kind::RequireKeyword),
    ("global", Kind::GlobalKeyword),
    ("return", Kind::ReturnKeyword),
    ("satisfies", Kind::SatisfiesKeyword),
    ("set", Kind::SetKeyword),
    ("static", Kind::StaticKeyword),
    ("string", Kind::StringKeyword),
    ("super", Kind::SuperKeyword),
    ("switch", Kind::SwitchKeyword),
    ("symbol", Kind::SymbolKeyword),
    ("this", Kind::ThisKeyword),
    ("throw", Kind::ThrowKeyword),
    ("true", Kind::TrueKeyword),
    ("try", Kind::TryKeyword),
    ("type", Kind::TypeKeyword),
    ("typeof", Kind::TypeOfKeyword),
    ("undefined", Kind::UndefinedKeyword),
    ("unique", Kind::UniqueKeyword),
    ("unknown", Kind::UnknownKeyword),
    ("using", Kind::UsingKeyword),
    ("var", Kind::VarKeyword),
    ("void", Kind::VoidKeyword),
    ("while", Kind::WhileKeyword),
    ("with", Kind::WithKeyword),
    ("yield", Kind::YieldKeyword),
    ("async", Kind::AsyncKeyword),
    ("await", Kind::AwaitKeyword),
    ("of", Kind::OfKeyword),
];

const TEXT_TO_PUNCTUATION: &[(&str, Kind)] = &[
    ("{", Kind::OpenBraceToken),
    ("}", Kind::CloseBraceToken),
    ("(", Kind::OpenParenToken),
    (")", Kind::CloseParenToken),
    ("[", Kind::OpenBracketToken),
    ("]", Kind::CloseBracketToken),
    (".", Kind::DotToken),
    ("...", Kind::DotDotDotToken),
    (";", Kind::SemicolonToken),
    (",", Kind::CommaToken),
    ("<", Kind::LessThanToken),
    (">", Kind::GreaterThanToken),
    ("<=", Kind::LessThanEqualsToken),
    (">=", Kind::GreaterThanEqualsToken),
    ("==", Kind::EqualsEqualsToken),
    ("!=", Kind::ExclamationEqualsToken),
    ("===", Kind::EqualsEqualsEqualsToken),
    ("!==", Kind::ExclamationEqualsEqualsToken),
    ("=>", Kind::EqualsGreaterThanToken),
    ("+", Kind::PlusToken),
    ("-", Kind::MinusToken),
    ("**", Kind::AsteriskAsteriskToken),
    ("*", Kind::AsteriskToken),
    ("/", Kind::SlashToken),
    ("%", Kind::PercentToken),
    ("++", Kind::PlusPlusToken),
    ("--", Kind::MinusMinusToken),
    ("<<", Kind::LessThanLessThanToken),
    ("</", Kind::LessThanSlashToken),
    (">>", Kind::GreaterThanGreaterThanToken),
    (">>>", Kind::GreaterThanGreaterThanGreaterThanToken),
    ("&", Kind::AmpersandToken),
    ("|", Kind::BarToken),
    ("^", Kind::CaretToken),
    ("!", Kind::ExclamationToken),
    ("~", Kind::TildeToken),
    ("&&", Kind::AmpersandAmpersandToken),
    ("||", Kind::BarBarToken),
    ("?", Kind::QuestionToken),
    ("??", Kind::QuestionQuestionToken),
    ("?.", Kind::QuestionDotToken),
    (":", Kind::ColonToken),
    ("=", Kind::EqualsToken),
    ("+=", Kind::PlusEqualsToken),
    ("-=", Kind::MinusEqualsToken),
    ("*=", Kind::AsteriskEqualsToken),
    ("**=", Kind::AsteriskAsteriskEqualsToken),
    ("/=", Kind::SlashEqualsToken),
    ("%=", Kind::PercentEqualsToken),
    ("<<=", Kind::LessThanLessThanEqualsToken),
    (">>=", Kind::GreaterThanGreaterThanEqualsToken),
    (">>>=", Kind::GreaterThanGreaterThanGreaterThanEqualsToken),
    ("&=", Kind::AmpersandEqualsToken),
    ("|=", Kind::BarEqualsToken),
    ("^=", Kind::CaretEqualsToken),
    ("||=", Kind::BarBarEqualsToken),
    ("&&=", Kind::AmpersandAmpersandEqualsToken),
    ("??=", Kind::QuestionQuestionEqualsToken),
    ("@", Kind::AtToken),
    ("#", Kind::HashToken),
    ("`", Kind::BacktickToken),
];

/// `SkipTrivia`: the position of the first non-trivia character at or after `pos`.
pub fn skip_trivia(text: &str, mut pos: usize) -> usize {
    let bytes = text.as_bytes();
    let len = bytes.len();
    loop {
        if pos >= len {
            return pos;
        }
        let (ch, size) = decode_rune(text, pos, len);
        match ch {
            0x0D => {
                if pos + 1 < len && bytes[pos + 1] == b'\n' {
                    pos += 1;
                }
                pos += 1;
                continue;
            }
            0x0A => {
                pos += 1;
                continue;
            }
            0x09 | 0x0B | 0x0C | 0x20 => {
                pos += 1;
                continue;
            }
            0x2F => {
                if pos + 1 < len {
                    if bytes[pos + 1] == b'/' {
                        pos += 2;
                        while pos < len {
                            let (ch, size) = decode_rune(text, pos, len);
                            if chars::is_line_break(ch) {
                                break;
                            }
                            pos += size;
                        }
                        continue;
                    }
                    if bytes[pos + 1] == b'*' {
                        pos += 2;
                        while pos < len {
                            if bytes[pos] == b'*' && pos + 1 < len && bytes[pos + 1] == b'/' {
                                pos += 2;
                                break;
                            }
                            let (_, size) = decode_rune(text, pos, len);
                            pos += size;
                        }
                        continue;
                    }
                }
            }
            0x3C | 0x7C | 0x3D | 0x3E => {
                if is_conflict_marker_trivia(text, pos) {
                    pos = scan_conflict_marker_trivia(text, pos);
                    continue;
                }
            }
            0x23 => {
                if pos == 0 && len >= 2 && bytes[1] == b'!' {
                    pos = 2;
                    while pos < len {
                        let (ch, size) = decode_rune(text, pos, len);
                        if chars::is_line_break(ch) {
                            break;
                        }
                        pos += size;
                    }
                    continue;
                }
            }
            _ => {
                if ch > 0x7F && chars::is_white_space_like(ch) {
                    pos += size;
                    continue;
                }
            }
        }
        return pos;
    }
}
