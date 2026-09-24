//! typescript-go's regular expression validator (`scanner/regexp.go`) and
//! its Unicode property tables (`scanner/unicodeproperties.go`), with the
//! checker's `checkGrammarRegularExpressionLiteral` that runs it.

use std::collections::{BTreeSet, HashSet};

use crate::chars;
use crate::messages as diagnostics;
use crate::parser::get_spelling_suggestion_for_strings;
use crate::scanner::{
    ESCAPE_ANNEX_B, ESCAPE_ANY_UNICODE_MODE, ESCAPE_ATOM_ESCAPE, ESCAPE_REGULAR_EXPRESSION, Scanner,
    decode_js_string_rune, encode_js_string_rune, go_decode_rune, go_rune_string,
};
use crate::{Diagnostic, Message, ScriptTarget};

/// The checker's `checkGrammarRegularExpressionLiteral` for the regular
/// expression literal that starts at `start` in `text`.
pub(crate) fn regular_expression_literal_diagnostics(
    text: &str,
    jsx: bool,
    language_version: ScriptTarget,
    start: usize,
) -> Vec<Diagnostic> {
    let mut scanner = Scanner::new(text, jsx);
    scanner.language_version = language_version;
    scanner.reset_token_state(start);
    scanner.scan();
    scanner.re_scan_slash_token(true);
    // The checker keeps the first error at each start; a `Did_you_mean_0` at
    // the last error's start and length becomes that error's related
    // information, and any other error at that start is dropped. The
    // scanner's own same-start dedup reports exactly those diagnostics.
    scanner.diagnostics
}

pub(crate) type RegularExpressionFlags = u32;

pub(crate) const FLAGS_NONE: RegularExpressionFlags = 0;
const FLAGS_HAS_INDICES: RegularExpressionFlags = 1 << 0; // d
const FLAGS_GLOBAL: RegularExpressionFlags = 1 << 1; // g
const FLAGS_IGNORE_CASE: RegularExpressionFlags = 1 << 2; // i
const FLAGS_MULTILINE: RegularExpressionFlags = 1 << 3; // m
const FLAGS_DOT_ALL: RegularExpressionFlags = 1 << 4; // s
const FLAGS_UNICODE: RegularExpressionFlags = 1 << 5; // u
const FLAGS_UNICODE_SETS: RegularExpressionFlags = 1 << 6; // v
const FLAGS_STICKY: RegularExpressionFlags = 1 << 7; // y
pub(crate) const FLAGS_ANY_UNICODE_MODE: RegularExpressionFlags = FLAGS_UNICODE | FLAGS_UNICODE_SETS;
const FLAGS_MODIFIERS: RegularExpressionFlags = FLAGS_IGNORE_CASE | FLAGS_MULTILINE | FLAGS_DOT_ALL;

/// `charCodeToRegExpFlag`.
pub(crate) fn char_code_to_reg_exp_flag(ch: i32) -> Option<RegularExpressionFlags> {
    match u8::try_from(ch).ok()? {
        b'd' => Some(FLAGS_HAS_INDICES),
        b'g' => Some(FLAGS_GLOBAL),
        b'i' => Some(FLAGS_IGNORE_CASE),
        b'm' => Some(FLAGS_MULTILINE),
        b's' => Some(FLAGS_DOT_ALL),
        b'u' => Some(FLAGS_UNICODE),
        b'v' => Some(FLAGS_UNICODE_SETS),
        b'y' => Some(FLAGS_STICKY),
        _ => None,
    }
}

/// `regExpFlagToFirstAvailableLanguageVersion`.
fn reg_exp_flag_to_first_available_language_version(flag: RegularExpressionFlags) -> Option<ScriptTarget> {
    match flag {
        FLAGS_HAS_INDICES => Some(ScriptTarget::ES2022),
        FLAGS_DOT_ALL => Some(ScriptTarget::ES2018),
        FLAGS_UNICODE_SETS => Some(ScriptTarget::ES2024),
        _ => None,
    }
}

impl Scanner<'_> {
    pub(crate) fn check_regular_expression_flag_availability(
        &mut self,
        flag: RegularExpressionFlags,
        pos: usize,
        size: usize,
    ) {
        if let Some(available_from) = reg_exp_flag_to_first_available_language_version(flag)
            && self.language_version < available_from
        {
            self.error_at(
                diagnostics::This_regular_expression_flag_is_only_available_when_targeting_0_or_later,
                pos,
                size,
                vec![available_from.name().to_string()],
            );
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum ClassSetExpressionType {
    Unknown,
    ClassUnion,
    ClassIntersection,
    ClassSubtraction,
}

struct GroupNameReference {
    pos: usize,
    end: usize,
    name: String,
}

struct DecimalEscapeValue {
    pos: usize,
    end: usize,
    value: i64,
}

/// Go's `char()` is a byte as a rune, or -1; this is the ASCII byte to match
/// on, and 0xFF (no ASCII pattern) for anything else.
fn ascii(ch: i32) -> u8 {
    if (0..0x80).contains(&ch) { ch as u8 } else { 0xFF }
}

/// Go's `string(ch)` of a message argument.
fn rune_arg(ch: i32) -> String {
    String::from_utf8_lossy(&go_rune_string(ch)).into_owned()
}

pub(crate) struct RegExpParser<'s, 'a> {
    scanner: &'s mut Scanner<'a>,
    end: usize,
    any_unicode_mode: bool,
    unicode_sets_mode: bool,
    annex_b: bool,

    any_unicode_mode_or_non_annex_b: bool,
    named_capture_groups: bool,

    /// See `scan_class_set_expression`.
    may_contain_strings: bool,
    /// The number of all (named and unnamed) capturing groups defined in the regex.
    number_of_capturing_groups: i64,
    /// All named capturing groups defined in the regex.
    // port: a Go map; only its size and an order-independent spelling
    // suggestion read it.
    group_specifiers: BTreeSet<String>,
    /// All references to named capturing groups in the regex.
    group_name_references: Vec<GroupNameReference>,
    /// All numeric backreferences within the regex.
    decimal_escapes: Vec<DecimalEscapeValue>,
    /// A stack of scopes for named capturing groups. See `scan_group_name`.
    named_capturing_groups: Vec<HashSet<String>>,

    /// The low surrogate to emit on the next `scan_source_character` call when
    /// a non-BMP character is split into UTF-16 code units in non-unicode mode.
    pending_low_surrogate: i32,
}

impl<'s, 'a> RegExpParser<'s, 'a> {
    pub(crate) fn new(
        scanner: &'s mut Scanner<'a>,
        end: usize,
        reg_exp_flags: RegularExpressionFlags,
        named_capture_groups: bool,
    ) -> Self {
        Self {
            scanner,
            end,
            any_unicode_mode: reg_exp_flags & FLAGS_ANY_UNICODE_MODE != 0,
            unicode_sets_mode: reg_exp_flags & FLAGS_UNICODE_SETS != 0,
            annex_b: true,
            any_unicode_mode_or_non_annex_b: false,
            named_capture_groups,
            may_contain_strings: false,
            number_of_capturing_groups: 0,
            group_specifiers: BTreeSet::new(),
            group_name_references: Vec::new(),
            decimal_escapes: Vec::new(),
            named_capturing_groups: Vec::new(),
            pending_low_surrogate: 0,
        }
    }

    fn pos(&self) -> usize {
        self.scanner.state.pos
    }

    fn inc_pos(&mut self, n: isize) {
        self.scanner.state.pos = self.scanner.state.pos.wrapping_add_signed(n);
    }

    fn char(&self) -> i32 {
        self.scanner.char()
    }

    fn char_at(&self, pos: usize) -> i32 {
        self.scanner.byte_at(pos)
    }

    fn error(&mut self, msg: &'static Message, pos: usize, length: usize, args: Vec<String>) {
        self.scanner.error_at(msg, pos, length, args);
    }

    fn text(&self) -> &'a str {
        self.scanner.text
    }

    fn bytes(&self) -> &'a [u8] {
        self.scanner.text.as_bytes()
    }

    /// `text[pos:pos+2]` when two bytes remain before `end`, else `""`.
    fn two_chars(&self) -> &'a [u8] {
        if self.pos() + 1 < self.end { &self.bytes()[self.pos()..self.pos() + 2] } else { b"" }
    }

    // Disjunction ::= Alternative ('|' Alternative)*
    fn scan_disjunction(&mut self, is_in_group: bool) {
        loop {
            self.named_capturing_groups.push(HashSet::new());
            self.scan_alternative(is_in_group);
            self.named_capturing_groups.pop();
            if self.char() != '|' as i32 {
                return;
            }
            self.inc_pos(1);
        }
    }

    // Alternative ::= Term*
    // Term ::=
    //     | Assertion
    //     | Atom Quantifier?
    // Assertion ::=
    //     | '^'
    //     | '$'
    //     | '\b'
    //     | '\B'
    //     | '(?=' Disjunction ')'
    //     | '(?!' Disjunction ')'
    //     | '(?<=' Disjunction ')'
    //     | '(?<!' Disjunction ')'
    // Quantifier ::= QuantifierPrefix '?'?
    // QuantifierPrefix ::=
    //     | '*'
    //     | '+'
    //     | '?'
    //     | '{' DecimalDigits (',' DecimalDigits?)? '}'
    // Atom ::=
    //     | PatternCharacter
    //     | '.'
    //     | '\' AtomEscape
    //     | CharacterClass
    //     | '(?<' RegExpIdentifierName '>' Disjunction ')'
    //     | '(?' RegularExpressionFlags ('-' RegularExpressionFlags)? ':' Disjunction ')'
    // CharacterClass ::= unicodeMode
    //     ? '[' ClassRanges ']'
    //     : '[' ClassSetExpression ']'
    fn scan_alternative(&mut self, is_in_group: bool) {
        let mut is_previous_term_quantifiable = false;
        while self.pos() < self.end {
            let start = self.pos();
            let ch = self.char();
            match ascii(ch) {
                b'^' | b'$' => {
                    self.inc_pos(1);
                    is_previous_term_quantifiable = false;
                }
                b'\\' => {
                    self.inc_pos(1);
                    match ascii(self.char()) {
                        b'b' | b'B' => {
                            self.inc_pos(1);
                            is_previous_term_quantifiable = false;
                        }
                        _ => {
                            self.scan_atom_escape();
                            is_previous_term_quantifiable = true;
                        }
                    }
                }
                b'(' => {
                    self.inc_pos(1);
                    if self.char() == '?' as i32 {
                        self.inc_pos(1);
                        match ascii(self.char()) {
                            b'=' | b'!' => {
                                self.inc_pos(1);
                                // In Annex B, `(?=Disjunction)` and `(?!Disjunction)` are quantifiable
                                is_previous_term_quantifiable = !self.any_unicode_mode_or_non_annex_b;
                            }
                            b'<' => {
                                let group_name_start = self.pos();
                                self.inc_pos(1);
                                match ascii(self.char()) {
                                    b'=' | b'!' => {
                                        self.inc_pos(1);
                                        is_previous_term_quantifiable = false;
                                    }
                                    _ => {
                                        self.scan_group_name(false);
                                        self.scan_expected_char('>');
                                        if self.scanner.language_version < ScriptTarget::ES2018 {
                                            self.error(
                                                diagnostics::Named_capturing_groups_are_only_available_when_targeting_ES2018_or_later,
                                                group_name_start,
                                                self.pos() - group_name_start,
                                                Vec::new(),
                                            );
                                        }
                                        self.number_of_capturing_groups += 1;
                                        is_previous_term_quantifiable = true;
                                    }
                                }
                            }
                            _ => {
                                let flags_start = self.pos();
                                let set_flags = self.scan_pattern_modifiers(FLAGS_NONE);
                                if self.char() == '-' as i32 {
                                    self.inc_pos(1);
                                    self.scan_pattern_modifiers(set_flags);
                                    if self.pos() == flags_start + 1 {
                                        self.error(
                                            diagnostics::Subpattern_flags_must_be_present_when_there_is_a_minus_sign,
                                            flags_start,
                                            self.pos() - flags_start,
                                            Vec::new(),
                                        );
                                    }
                                }
                                self.scan_expected_char(':');
                                is_previous_term_quantifiable = true;
                            }
                        }
                    } else {
                        self.number_of_capturing_groups += 1;
                        is_previous_term_quantifiable = true;
                    }
                    self.scan_disjunction(true);
                    self.scan_expected_char(')');
                }
                b'{' => {
                    self.inc_pos(1);
                    let digits_start = self.pos();
                    self.scan_digits();
                    let min_str = self.scanner.state.token_value.clone();
                    if !self.any_unicode_mode_or_non_annex_b && min_str.is_empty() {
                        is_previous_term_quantifiable = true;
                        continue;
                    }
                    if self.char() == ',' as i32 {
                        self.inc_pos(1);
                        self.scan_digits();
                        let max_str = self.scanner.state.token_value.clone();
                        if min_str.is_empty() {
                            if !max_str.is_empty() || self.char() == '}' as i32 {
                                self.error(diagnostics::Incomplete_quantifier_Digit_expected, digits_start, 0, Vec::new());
                            } else {
                                self.error(
                                    diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                                    start,
                                    1,
                                    vec![rune_arg(ch)],
                                );
                                is_previous_term_quantifiable = true;
                                continue;
                            }
                        } else if !max_str.is_empty()
                            && compare_decimal_strings(&min_str, &max_str) == std::cmp::Ordering::Greater
                            && (self.any_unicode_mode_or_non_annex_b || self.char() == '}' as i32)
                        {
                            self.error(
                                diagnostics::Numbers_out_of_order_in_quantifier,
                                digits_start,
                                self.pos() - digits_start,
                                Vec::new(),
                            );
                        }
                    } else if min_str.is_empty() {
                        if self.any_unicode_mode_or_non_annex_b {
                            self.error(
                                diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                                start,
                                1,
                                vec![rune_arg(ch)],
                            );
                        }
                        is_previous_term_quantifiable = true;
                        continue;
                    }
                    if self.char() != '}' as i32 {
                        if self.any_unicode_mode_or_non_annex_b {
                            self.error(diagnostics::X_0_expected, self.pos(), 0, vec!["}".to_string()]);
                            self.inc_pos(-1);
                        } else {
                            is_previous_term_quantifiable = true;
                            continue;
                        }
                    }
                    // port: Go falls through to the `'*', '+', '?'` case.
                    self.scan_quantifier(start, &mut is_previous_term_quantifiable);
                }
                b'*' | b'+' | b'?' => self.scan_quantifier(start, &mut is_previous_term_quantifiable),
                b'.' => {
                    self.inc_pos(1);
                    is_previous_term_quantifiable = true;
                }
                b'[' => {
                    self.inc_pos(1);
                    if self.unicode_sets_mode {
                        self.scan_class_set_expression();
                    } else {
                        self.scan_class_ranges();
                        self.pending_low_surrogate = 0;
                    }
                    self.scan_expected_char(']');
                    is_previous_term_quantifiable = true;
                }
                b')' if is_in_group => return,
                // port: Go's `case ')'` falls through here when not in a group.
                b')' | b']' | b'}' => {
                    if self.any_unicode_mode_or_non_annex_b || ch == ')' as i32 {
                        self.error(
                            diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                            self.pos(),
                            1,
                            vec![rune_arg(ch)],
                        );
                    }
                    self.inc_pos(1);
                    is_previous_term_quantifiable = true;
                }
                b'/' | b'|' => return,
                _ => {
                    self.scan_source_character();
                    is_previous_term_quantifiable = true;
                }
            }
        }
    }

    /// The `'*', '+', '?'` case of `scanAlternative`, which `'{'` falls
    /// through to.
    fn scan_quantifier(&mut self, start: usize, is_previous_term_quantifiable: &mut bool) {
        self.inc_pos(1);
        if self.char() == '?' as i32 {
            // Non-greedy
            self.inc_pos(1);
        }
        if !*is_previous_term_quantifiable {
            self.error(diagnostics::There_is_nothing_available_for_repetition, start, self.pos() - start, Vec::new());
        }
        *is_previous_term_quantifiable = false;
    }

    fn scan_pattern_modifiers(&mut self, mut curr_flags: RegularExpressionFlags) -> RegularExpressionFlags {
        while self.pos() < self.end {
            let (ch, size) = go_decode_rune(self.bytes(), self.pos());
            if ch == chars::RUNE_ERROR || !chars::is_identifier_part(ch) {
                break;
            }
            match char_code_to_reg_exp_flag(ch) {
                None => self.error(diagnostics::Unknown_regular_expression_flag, self.pos(), size, Vec::new()),
                Some(flag) if curr_flags & flag != 0 => {
                    self.error(diagnostics::Duplicate_regular_expression_flag, self.pos(), size, Vec::new());
                }
                Some(flag) if flag & FLAGS_MODIFIERS == 0 => {
                    self.error(
                        diagnostics::This_regular_expression_flag_cannot_be_toggled_within_a_subpattern,
                        self.pos(),
                        size,
                        Vec::new(),
                    );
                }
                Some(flag) => {
                    curr_flags |= flag;
                    let pos = self.pos();
                    self.scanner.check_regular_expression_flag_availability(flag, pos, size);
                }
            }
            self.inc_pos(size as isize);
        }
        curr_flags
    }

    // AtomEscape ::=
    //     | DecimalEscape
    //     | CharacterClassEscape
    //     | CharacterEscape
    //     | 'k<' RegExpIdentifierName '>'
    fn scan_atom_escape(&mut self) {
        match ascii(self.char()) {
            b'k' => {
                self.inc_pos(1);
                if self.char() == '<' as i32 {
                    self.inc_pos(1);
                    self.scan_group_name(true);
                    self.scan_expected_char('>');
                } else if self.any_unicode_mode_or_non_annex_b || self.named_capture_groups {
                    self.error(
                        diagnostics::X_k_must_be_followed_by_a_capturing_group_name_enclosed_in_angle_brackets,
                        self.pos() - 2,
                        2,
                        Vec::new(),
                    );
                }
            }
            // port: Go's `case 'q'` falls through to `default` outside Unicode Sets mode.
            b'q' if self.unicode_sets_mode => {
                self.inc_pos(1);
                self.error(diagnostics::X_q_is_only_available_inside_character_class, self.pos() - 2, 2, Vec::new());
            }
            _ => {
                if !self.scan_character_class_escape() && !self.scan_decimal_escape() {
                    // Regex literals cannot contain line breaks here, so a character escape must consume something.
                    // port: Go asserts the result is non-empty.
                    self.scan_character_escape(true);
                }
            }
        }
    }

    // DecimalEscape ::= [1-9] [0-9]*
    fn scan_decimal_escape(&mut self) -> bool {
        let ch = self.char();
        if ch >= '1' as i32 && ch <= '9' as i32 {
            let start = self.pos();
            self.scan_digits();
            let value = self.scanner.state.token_value.parse::<i64>().unwrap_or(i64::MAX);
            self.decimal_escapes.push(DecimalEscapeValue { pos: start, end: self.pos(), value });
            return true;
        }
        false
    }

    // CharacterEscape ::=
    //     | `c` ControlLetter
    //     | IdentityEscape
    //     | (Other sequences handled by `scanEscapeSequence`)
    // IdentityEscape ::=
    //     | '^' | '$' | '/' | '\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
    //     | [~AnyUnicodeMode] (any other non-identifier characters)
    fn scan_character_escape(&mut self, atom_escape: bool) -> Vec<u8> {
        let mut ch = self.char();
        if ch == -1 {
            self.error(diagnostics::Undetermined_character_escape, self.pos() - 1, 1, Vec::new());
            return b"\\".to_vec();
        }
        match ascii(ch) {
            b'c' => {
                self.inc_pos(1);
                ch = self.char();
                if chars::is_ascii_letter(ch) {
                    self.inc_pos(1);
                    return go_rune_string(ch & 0x1f);
                }
                if self.any_unicode_mode_or_non_annex_b {
                    self.error(diagnostics::X_c_must_be_followed_by_an_ASCII_letter, self.pos() - 2, 2, Vec::new());
                } else if atom_escape {
                    self.inc_pos(-1);
                    return b"\\".to_vec();
                }
                go_rune_string(ch)
            }
            b'^' | b'$' | b'/' | b'\\' | b'.' | b'*' | b'+' | b'?' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'|' => {
                self.inc_pos(1);
                go_rune_string(ch)
            }
            _ => {
                // back up to include the backslash for scanEscapeSequence
                self.inc_pos(-1);
                let mut flags = ESCAPE_REGULAR_EXPRESSION;
                if self.annex_b {
                    flags |= ESCAPE_ANNEX_B;
                }
                if self.any_unicode_mode {
                    flags |= ESCAPE_ANY_UNICODE_MODE;
                }
                if atom_escape {
                    flags |= ESCAPE_ATOM_ESCAPE;
                }
                self.scanner.scan_escape_sequence_bytes(flags)
            }
        }
    }

    fn scan_group_name(&mut self, is_reference: bool) {
        self.scanner.state.token_start = self.pos();
        self.scanner.scan_identifier(0);
        let token_start = self.scanner.state.token_start;
        if self.pos() == token_start {
            self.error(diagnostics::Expected_a_capturing_group_name, self.pos(), 0, Vec::new());
        } else if is_reference {
            let name = self.scanner.state.token_value.clone();
            self.group_name_references.push(GroupNameReference { pos: token_start, end: self.pos(), name });
        } else if self.named_capturing_groups_contains(&self.scanner.state.token_value) {
            self.error(
                diagnostics::Named_capturing_groups_with_the_same_name_must_be_mutually_exclusive_to_each_other,
                token_start,
                self.pos() - token_start,
                Vec::new(),
            );
        } else {
            let name = self.scanner.state.token_value.clone();
            if let Some(scope) = self.named_capturing_groups.last_mut() {
                scope.insert(name.clone());
            }
            self.group_specifiers.insert(name);
        }
    }

    fn named_capturing_groups_contains(&self, name: &str) -> bool {
        self.named_capturing_groups.iter().any(|group| group.contains(name))
    }

    fn is_class_content_exit(&self, ch: i32) -> bool {
        ch == ']' as i32 || self.pos() >= self.end
    }

    // ClassRanges ::= '^'? (ClassAtom ('-' ClassAtom)?)*
    fn scan_class_ranges(&mut self) {
        self.pending_low_surrogate = 0;
        if self.char() == '^' as i32 {
            self.inc_pos(1);
        }
        while self.pos() < self.end {
            let mut ch = self.char();
            if self.is_class_content_exit(ch) {
                return;
            }
            let min_start = self.pos();
            let min_character = self.scan_class_atom();
            if self.char() == '-' as i32 {
                self.inc_pos(1);
                ch = self.char();
                if self.is_class_content_exit(ch) {
                    return;
                }
                if min_character.is_empty() && self.any_unicode_mode_or_non_annex_b {
                    self.error(
                        diagnostics::A_character_class_range_must_not_be_bounded_by_another_character_class,
                        min_start,
                        self.pos() - 1 - min_start,
                        Vec::new(),
                    );
                }
                let max_start = self.pos();
                let max_character = self.scan_class_atom();
                if max_character.is_empty() && self.any_unicode_mode_or_non_annex_b {
                    self.error(
                        diagnostics::A_character_class_range_must_not_be_bounded_by_another_character_class,
                        max_start,
                        self.pos() - max_start,
                        Vec::new(),
                    );
                    continue;
                }
                if min_character.is_empty() {
                    continue;
                }
                let (min_character_value, min_size) = decode_js_string_rune(&min_character);
                let (max_character_value, max_size) = decode_js_string_rune(&max_character);
                if min_character.len() == min_size
                    && max_character.len() == max_size
                    && min_character_value > max_character_value
                {
                    self.error(
                        diagnostics::Range_out_of_order_in_character_class,
                        min_start,
                        self.pos() - min_start,
                        Vec::new(),
                    );
                }
            }
        }
    }

    // Static Semantics: MayContainStrings
    //     ClassUnion: ClassSetOperands.some(ClassSetOperand => ClassSetOperand.MayContainStrings)
    //     ClassIntersection: ClassSetOperands.every(ClassSetOperand => ClassSetOperand.MayContainStrings)
    //     ClassSubtraction: ClassSetOperands[0].MayContainStrings
    //     ClassSetOperand:
    //         || ClassStringDisjunctionContents.MayContainStrings
    //         || CharacterClassEscape.UnicodePropertyValueExpression.LoneUnicodePropertyNameOrValue.MayContainStrings
    //     ClassStringDisjunctionContents: ClassStrings.some(ClassString => ClassString.ClassSetCharacters.length !== 1)
    //     LoneUnicodePropertyNameOrValue: isBinaryUnicodePropertyOfStrings(LoneUnicodePropertyNameOrValue)

    // ClassSetExpression ::= '^'? (ClassUnion | ClassIntersection | ClassSubtraction)
    // ClassUnion ::= (ClassSetRange | ClassSetOperand)*
    // ClassIntersection ::= ClassSetOperand ('&&' ClassSetOperand)+
    // ClassSubtraction ::= ClassSetOperand ('--' ClassSetOperand)+
    // ClassSetRange ::= ClassSetCharacter '-' ClassSetCharacter
    fn scan_class_set_expression(&mut self) {
        let mut is_character_complement = false;
        if self.char() == '^' as i32 {
            self.inc_pos(1);
            is_character_complement = true;
        }
        let mut expression_may_contain_strings = false;
        let mut ch = self.char();
        if self.is_class_content_exit(ch) {
            return;
        }
        let mut start = self.pos();
        let mut operand: Vec<u8> = Vec::new();
        match self.two_chars() {
            b"--" | b"&&" => {
                self.error(diagnostics::Expected_a_class_set_operand, self.pos(), 0, Vec::new());
                self.may_contain_strings = false;
            }
            _ => operand = self.scan_class_set_operand(),
        }
        match ascii(self.char()) {
            b'-' => {
                if self.pos() + 1 < self.end && self.char_at(self.pos() + 1) == '-' as i32 {
                    if is_character_complement && self.may_contain_strings {
                        self.error(
                            diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class,
                            start,
                            self.pos() - start,
                            Vec::new(),
                        );
                    }
                    expression_may_contain_strings = self.may_contain_strings;
                    self.scan_class_set_sub_expression(ClassSetExpressionType::ClassSubtraction);
                    self.may_contain_strings = !is_character_complement && expression_may_contain_strings;
                    return;
                }
            }
            b'&' => {
                if self.pos() + 1 < self.end && self.char_at(self.pos() + 1) == '&' as i32 {
                    self.scan_class_set_sub_expression(ClassSetExpressionType::ClassIntersection);
                    if is_character_complement && self.may_contain_strings {
                        self.error(
                            diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class,
                            start,
                            self.pos() - start,
                            Vec::new(),
                        );
                    }
                    expression_may_contain_strings = self.may_contain_strings;
                    self.may_contain_strings = !is_character_complement && expression_may_contain_strings;
                    return;
                } else {
                    // port: Go reports `ch`, the first character of the class, not the `&`.
                    self.error(
                        diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                        self.pos(),
                        1,
                        vec![rune_arg(ch)],
                    );
                }
            }
            _ => {
                if is_character_complement && self.may_contain_strings {
                    self.error(
                        diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class,
                        start,
                        self.pos() - start,
                        Vec::new(),
                    );
                }
                expression_may_contain_strings = self.may_contain_strings;
            }
        }
        while self.pos() < self.end {
            ch = self.char();
            match ascii(ch) {
                b'-' => {
                    self.inc_pos(1);
                    ch = self.char();
                    if self.is_class_content_exit(ch) {
                        self.may_contain_strings = !is_character_complement && expression_may_contain_strings;
                        return;
                    }
                    if ch == '-' as i32 {
                        self.inc_pos(1);
                        self.error(
                            diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead,
                            self.pos() - 2,
                            2,
                            Vec::new(),
                        );
                        start = self.pos() - 2;
                        operand = self.bytes()[start..self.pos()].to_vec();
                        continue;
                    } else {
                        if operand.is_empty() {
                            self.error(
                                diagnostics::A_character_class_range_must_not_be_bounded_by_another_character_class,
                                start,
                                self.pos() - 1 - start,
                                Vec::new(),
                            );
                        }
                        let second_start = self.pos();
                        let second_operand = self.scan_class_set_operand();
                        if is_character_complement && self.may_contain_strings {
                            self.error(
                                diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class,
                                second_start,
                                self.pos() - second_start,
                                Vec::new(),
                            );
                        }
                        expression_may_contain_strings = expression_may_contain_strings || self.may_contain_strings;
                        if second_operand.is_empty() {
                            self.error(
                                diagnostics::A_character_class_range_must_not_be_bounded_by_another_character_class,
                                second_start,
                                self.pos() - second_start,
                                Vec::new(),
                            );
                        } else if !operand.is_empty() {
                            let (min_character_value, min_size) = decode_js_string_rune(&operand);
                            let (max_character_value, max_size) = decode_js_string_rune(&second_operand);
                            if operand.len() == min_size
                                && second_operand.len() == max_size
                                && min_character_value > max_character_value
                            {
                                self.error(
                                    diagnostics::Range_out_of_order_in_character_class,
                                    start,
                                    self.pos() - start,
                                    Vec::new(),
                                );
                            }
                        }
                    }
                }
                b'&' => {
                    start = self.pos();
                    self.inc_pos(1);
                    if self.char() == '&' as i32 {
                        self.inc_pos(1);
                        self.error(
                            diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead,
                            self.pos() - 2,
                            2,
                            Vec::new(),
                        );
                        if self.char() == '&' as i32 {
                            self.error(
                                diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                                self.pos(),
                                1,
                                vec![rune_arg(ch)],
                            );
                            self.inc_pos(1);
                        }
                    } else {
                        self.error(
                            diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                            self.pos() - 1,
                            1,
                            vec![rune_arg(ch)],
                        );
                    }
                    operand = self.bytes()[start..self.pos()].to_vec();
                    continue;
                }
                _ => {}
            }
            if self.is_class_content_exit(self.char()) {
                break;
            }
            start = self.pos();
            match self.two_chars() {
                b"--" | b"&&" => {
                    self.error(
                        diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead,
                        self.pos(),
                        2,
                        Vec::new(),
                    );
                    self.inc_pos(2);
                    operand = self.bytes()[start..self.pos()].to_vec();
                }
                _ => operand = self.scan_class_set_operand(),
            }
        }
        self.may_contain_strings = !is_character_complement && expression_may_contain_strings;
    }

    fn scan_class_set_sub_expression(&mut self, expression_type: ClassSetExpressionType) {
        let mut expression_may_contain_strings = self.may_contain_strings;
        while self.pos() < self.end {
            let mut ch = self.char();
            if self.is_class_content_exit(ch) {
                break;
            }
            match ascii(ch) {
                b'-' => {
                    self.inc_pos(1);
                    if self.char() == '-' as i32 {
                        self.inc_pos(1);
                        if expression_type != ClassSetExpressionType::ClassSubtraction {
                            self.error(
                                diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead,
                                self.pos() - 2,
                                2,
                                Vec::new(),
                            );
                        }
                    } else {
                        self.error(
                            diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead,
                            self.pos() - 1,
                            1,
                            Vec::new(),
                        );
                    }
                }
                b'&' => {
                    self.inc_pos(1);
                    if self.char() == '&' as i32 {
                        self.inc_pos(1);
                        if expression_type != ClassSetExpressionType::ClassIntersection {
                            self.error(
                                diagnostics::Operators_must_not_be_mixed_within_a_character_class_Wrap_it_in_a_nested_class_instead,
                                self.pos() - 2,
                                2,
                                Vec::new(),
                            );
                        }
                        if self.char() == '&' as i32 {
                            self.error(
                                diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                                self.pos(),
                                1,
                                vec![rune_arg(ch)],
                            );
                            self.inc_pos(1);
                        }
                    } else {
                        self.error(
                            diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                            self.pos() - 1,
                            1,
                            vec![rune_arg(ch)],
                        );
                    }
                }
                _ => match expression_type {
                    ClassSetExpressionType::ClassSubtraction => {
                        self.error(diagnostics::X_0_expected, self.pos(), 0, vec!["--".to_string()]);
                    }
                    ClassSetExpressionType::ClassIntersection => {
                        self.error(diagnostics::X_0_expected, self.pos(), 0, vec!["&&".to_string()]);
                    }
                    _ => {}
                },
            }
            ch = self.char();
            if self.is_class_content_exit(ch) {
                self.error(diagnostics::Expected_a_class_set_operand, self.pos(), 0, Vec::new());
                break;
            }
            self.scan_class_set_operand();
            if expression_type == ClassSetExpressionType::ClassIntersection {
                expression_may_contain_strings = expression_may_contain_strings && self.may_contain_strings;
            }
        }
        self.may_contain_strings = expression_may_contain_strings;
    }

    // ClassSetOperand ::=
    //     | '[' ClassSetExpression ']'
    //     | '\' CharacterClassEscape
    //     | '\q{' ClassStringDisjunctionContents '}'
    //     | ClassSetCharacter
    fn scan_class_set_operand(&mut self) -> Vec<u8> {
        self.may_contain_strings = false;
        match ascii(self.char()) {
            b'[' => {
                self.inc_pos(1);
                self.scan_class_set_expression();
                self.scan_expected_char(']');
                Vec::new()
            }
            b'\\' => {
                self.inc_pos(1);
                if self.scan_character_class_escape() {
                    return Vec::new();
                } else if self.char() == 'q' as i32 {
                    self.inc_pos(1);
                    if self.char() == '{' as i32 {
                        self.inc_pos(1);
                        self.scan_class_string_disjunction_contents();
                        self.scan_expected_char('}');
                        return Vec::new();
                    } else {
                        self.error(
                            diagnostics::X_q_must_be_followed_by_string_alternatives_enclosed_in_braces,
                            self.pos() - 2,
                            2,
                            Vec::new(),
                        );
                        return b"q".to_vec();
                    }
                }
                self.inc_pos(-1);
                // port: Go falls through to `default`.
                self.scan_class_set_character()
            }
            _ => self.scan_class_set_character(),
        }
    }

    // ClassStringDisjunctionContents ::= ClassSetCharacter* ('|' ClassSetCharacter*)*
    fn scan_class_string_disjunction_contents(&mut self) {
        let mut character_count = 0;
        while self.pos() < self.end {
            let ch = self.char();
            match ascii(ch) {
                b'}' => {
                    if character_count != 1 {
                        self.may_contain_strings = true;
                    }
                    return;
                }
                b'|' => {
                    if character_count != 1 {
                        self.may_contain_strings = true;
                    }
                    self.inc_pos(1);
                    character_count = 0;
                }
                _ => {
                    self.scan_class_set_character();
                    character_count += 1;
                }
            }
        }
    }

    // ClassSetCharacter ::=
    //     | SourceCharacter -- ClassSetSyntaxCharacter -- ClassSetReservedDoublePunctuator
    //     | '\' (CharacterEscape | ClassSetReservedPunctuator | 'b')
    fn scan_class_set_character(&mut self) -> Vec<u8> {
        let ch = self.char();
        if ch == '\\' as i32 {
            self.inc_pos(1);
            let inner_ch = self.char();
            match ascii(inner_ch) {
                b'b' => {
                    self.inc_pos(1);
                    return vec![0x08];
                }
                b'&' | b'-' | b'!' | b'#' | b'%' | b',' | b':' | b';' | b'<' | b'=' | b'>' | b'@' | b'`' | b'~' => {
                    self.inc_pos(1);
                    return go_rune_string(inner_ch);
                }
                _ => return self.scan_character_escape(false),
            }
        } else if self.pos() + 1 < self.end && ch == self.char_at(self.pos() + 1) {
            match ascii(ch) {
                b'&' | b'!' | b'#' | b'%' | b'*' | b'+' | b',' | b'.' | b':' | b';' | b'<' | b'=' | b'>' | b'?'
                | b'@' | b'`' | b'~' => {
                    self.error(
                        diagnostics::A_character_class_must_not_contain_a_reserved_double_punctuator_Did_you_mean_to_escape_it_with_backslash,
                        self.pos(),
                        2,
                        Vec::new(),
                    );
                    self.inc_pos(2);
                    return self.bytes()[self.pos() - 2..self.pos()].to_vec();
                }
                _ => {}
            }
        }
        match ascii(ch) {
            b'/' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'-' | b'|' => {
                self.error(
                    diagnostics::Unexpected_0_Did_you_mean_to_escape_it_with_backslash,
                    self.pos(),
                    1,
                    vec![rune_arg(ch)],
                );
                self.inc_pos(1);
                return go_rune_string(ch);
            }
            _ => {}
        }
        self.scan_source_character()
    }

    // ClassAtom ::=
    //     | SourceCharacter but not one of '\' or ']'
    //     | '\' ClassEscape
    // ClassEscape ::=
    //     | 'b'
    //     | '-'
    //     | CharacterClassEscape
    //     | CharacterEscape
    fn scan_class_atom(&mut self) -> Vec<u8> {
        if self.char() == '\\' as i32 {
            self.inc_pos(1);
            let ch = self.char();
            match ascii(ch) {
                b'b' => {
                    self.inc_pos(1);
                    vec![0x08]
                }
                b'-' => {
                    self.inc_pos(1);
                    go_rune_string(ch)
                }
                _ => {
                    if self.scan_character_class_escape() {
                        return Vec::new();
                    }
                    self.scan_character_escape(false)
                }
            }
        } else {
            self.scan_source_character()
        }
    }

    // CharacterClassEscape ::=
    //     | 'd' | 'D' | 's' | 'S' | 'w' | 'W'
    //     | [+AnyUnicodeMode] ('P' | 'p') '{' UnicodePropertyValueExpression '}'
    fn scan_character_class_escape(&mut self) -> bool {
        let mut is_character_complement = false;
        let start = self.pos() - 1;
        let ch = self.char();
        match ascii(ch) {
            b'd' | b'D' | b's' | b'S' | b'w' | b'W' => {
                self.inc_pos(1);
                true
            }
            // port: Go's `case 'P'` sets the complement and falls through to `case 'p'`.
            b'P' | b'p' => {
                if ch == 'P' as i32 {
                    is_character_complement = true;
                }
                self.inc_pos(1);
                if self.char() == '{' as i32 {
                    self.inc_pos(1);
                    let property_name_or_value_start = self.pos();
                    let property_name_or_value = self.scan_word_characters();
                    if self.char() == '=' as i32 {
                        let property_name = non_binary_unicode_property(property_name_or_value);
                        if self.pos() == property_name_or_value_start {
                            self.error(diagnostics::Expected_a_Unicode_property_name, self.pos(), 0, Vec::new());
                        } else if property_name.is_empty() {
                            self.error(
                                diagnostics::Unknown_Unicode_property_name,
                                property_name_or_value_start,
                                self.pos() - property_name_or_value_start,
                                Vec::new(),
                            );
                            let suggestion = self.get_spelling_suggestion_for_unicode_property_name(property_name_or_value);
                            if !suggestion.is_empty() {
                                self.error(
                                    diagnostics::Did_you_mean_0,
                                    property_name_or_value_start,
                                    self.pos() - property_name_or_value_start,
                                    vec![suggestion],
                                );
                            }
                        }
                        self.inc_pos(1);
                        let property_value_start = self.pos();
                        let property_value = self.scan_word_characters();
                        if self.pos() == property_value_start {
                            self.error(diagnostics::Expected_a_Unicode_property_value, self.pos(), 0, Vec::new());
                        } else if !property_name.is_empty() {
                            let values = values_of_non_binary_unicode_properties(property_name);
                            if values.is_some_and(|values| !values.contains(&property_value)) {
                                self.error(
                                    diagnostics::Unknown_Unicode_property_value,
                                    property_value_start,
                                    self.pos() - property_value_start,
                                    Vec::new(),
                                );
                                let suggestion =
                                    self.get_spelling_suggestion_for_unicode_property_value(property_name, property_value);
                                if !suggestion.is_empty() {
                                    self.error(
                                        diagnostics::Did_you_mean_0,
                                        property_value_start,
                                        self.pos() - property_value_start,
                                        vec![suggestion],
                                    );
                                }
                            }
                        }
                    } else if self.pos() == property_name_or_value_start {
                        self.error(diagnostics::Expected_a_Unicode_property_name_or_value, self.pos(), 0, Vec::new());
                    } else if BINARY_UNICODE_PROPERTIES_OF_STRINGS.contains(&property_name_or_value) {
                        if !self.unicode_sets_mode {
                            self.error(
                                diagnostics::Any_Unicode_property_that_would_possibly_match_more_than_a_single_character_is_only_available_when_the_Unicode_Sets_v_flag_is_set,
                                property_name_or_value_start,
                                self.pos() - property_name_or_value_start,
                                Vec::new(),
                            );
                        } else if is_character_complement {
                            self.error(
                                diagnostics::Anything_that_would_possibly_match_more_than_a_single_character_is_invalid_inside_a_negated_character_class,
                                property_name_or_value_start,
                                self.pos() - property_name_or_value_start,
                                Vec::new(),
                            );
                        } else {
                            self.may_contain_strings = true;
                        }
                    } else if !GENERAL_CATEGORY_VALUES.contains(&property_name_or_value)
                        && !BINARY_UNICODE_PROPERTIES.contains(&property_name_or_value)
                    {
                        self.error(
                            diagnostics::Unknown_Unicode_property_name_or_value,
                            property_name_or_value_start,
                            self.pos() - property_name_or_value_start,
                            Vec::new(),
                        );
                        let suggestion =
                            self.get_spelling_suggestion_for_unicode_property_name_or_value(property_name_or_value);
                        if !suggestion.is_empty() {
                            self.error(
                                diagnostics::Did_you_mean_0,
                                property_name_or_value_start,
                                self.pos() - property_name_or_value_start,
                                vec![suggestion],
                            );
                        }
                    }
                    self.scan_expected_char('}');
                    if !self.any_unicode_mode {
                        self.error(
                            diagnostics::Unicode_property_value_expressions_are_only_available_when_the_Unicode_u_flag_or_the_Unicode_Sets_v_flag_is_set,
                            start,
                            self.pos() - start,
                            Vec::new(),
                        );
                    }
                } else if self.any_unicode_mode_or_non_annex_b {
                    self.error(
                        diagnostics::X_0_must_be_followed_by_a_Unicode_property_value_expression_enclosed_in_braces,
                        self.pos() - 2,
                        2,
                        vec![rune_arg(ch)],
                    );
                } else {
                    self.inc_pos(-1);
                    return false;
                }
                true
            }
            _ => false,
        }
    }

    fn get_spelling_suggestion_for_unicode_property_name(&self, name: &str) -> String {
        let candidates: Vec<&str> = NON_BINARY_UNICODE_PROPERTIES.iter().map(|&(alias, _)| alias).collect();
        get_spelling_suggestion_for_strings(name, &candidates)
    }

    fn get_spelling_suggestion_for_unicode_property_value(&self, property_name: &str, value: &str) -> String {
        let Some(values) = values_of_non_binary_unicode_properties(property_name) else {
            return String::new();
        };
        get_spelling_suggestion_for_strings(value, values)
    }

    fn get_spelling_suggestion_for_unicode_property_name_or_value(&self, name: &str) -> String {
        let candidates: Vec<&str> = GENERAL_CATEGORY_VALUES
            .iter()
            .chain(BINARY_UNICODE_PROPERTIES)
            .chain(BINARY_UNICODE_PROPERTIES_OF_STRINGS)
            .copied()
            .collect();
        get_spelling_suggestion_for_strings(name, &candidates)
    }

    fn scan_word_characters(&mut self) -> &'a str {
        let start = self.pos();
        while self.pos() < self.end {
            let ch = self.char();
            if !is_word_character(ch) {
                break;
            }
            self.inc_pos(1);
        }
        &self.text()[start..self.pos()]
    }

    fn scan_source_character(&mut self) -> Vec<u8> {
        if self.pos() >= self.end {
            return Vec::new();
        }
        if !self.any_unicode_mode {
            if self.pending_low_surrogate != 0 {
                // Second of two surrogate code units for the same non-BMP character.
                // Now advance past the full UTF-8 sequence (the high surrogate call did not advance).
                let (_, size) = go_decode_rune(self.bytes(), self.pos());
                self.inc_pos(size as isize);
                let low = self.pending_low_surrogate;
                self.pending_low_surrogate = 0;
                return encode_js_string_rune(low);
            }
            let (ch, size) = go_decode_rune(self.bytes(), self.pos());
            if ch == chars::RUNE_ERROR || size == 0 {
                // Not a valid rune; consume one raw byte.
                self.inc_pos(1);
                return go_rune_string(self.bytes()[self.pos() - 1] as i32);
            }
            if ch >= 0x10000 {
                // Non-BMP character: emit the high surrogate first WITHOUT advancing.
                // The low surrogate will be emitted on the next call, which also advances.
                let value = ch - 0x10000;
                let high = 0xD800 + ((value >> 10) & 0x3FF);
                let low = 0xDC00 + (value & 0x3FF);
                self.pending_low_surrogate = low;
                return encode_js_string_rune(high);
            }
            self.inc_pos(size as isize);
            return go_rune_string(ch);
        }
        let (ch, size) = go_decode_rune(self.bytes(), self.pos());
        if size == 0 {
            return Vec::new();
        }
        if ch == chars::RUNE_ERROR {
            // Invalid UTF-8; consume the byte to avoid infinite loops.
            self.inc_pos(size as isize);
            return Vec::new();
        }
        self.inc_pos(size as isize);
        go_rune_string(ch)
    }

    fn scan_expected_char(&mut self, ch: char) {
        if self.char() == ch as i32 {
            self.inc_pos(1);
        } else {
            self.error(diagnostics::X_0_expected, self.pos(), 0, vec![ch.to_string()]);
        }
    }

    fn scan_digits(&mut self) {
        let start = self.pos();
        while self.pos() < self.end && chars::is_digit(self.char()) {
            self.inc_pos(1);
        }
        self.scanner.state.token_value = self.text()[start..self.pos()].to_string();
    }

    pub(crate) fn run(mut self) {
        // Regular expressions are checked more strictly when either in 'u' or 'v' mode, or
        // when not using the looser interpretation of the syntax from ECMA-262 Annex B.
        self.any_unicode_mode_or_non_annex_b = self.any_unicode_mode || !self.annex_b;

        self.scan_disjunction(false);

        for index in 0..self.group_name_references.len() {
            let GroupNameReference { pos, end, ref name } = self.group_name_references[index];
            if !self.group_specifiers.contains(name) {
                let name = name.clone();
                self.error(
                    diagnostics::There_is_no_capturing_group_named_0_in_this_regular_expression,
                    pos,
                    end - pos,
                    vec![name.clone()],
                );
                if !self.group_specifiers.is_empty() {
                    let candidates: Vec<&str> = self.group_specifiers.iter().map(String::as_str).collect();
                    let suggestion = get_spelling_suggestion_for_strings(&name, &candidates);
                    if !suggestion.is_empty() {
                        self.error(diagnostics::Did_you_mean_0, pos, end - pos, vec![suggestion]);
                    }
                }
            }
        }
        for index in 0..self.decimal_escapes.len() {
            let DecimalEscapeValue { pos, end, value } = self.decimal_escapes[index];
            // Although a DecimalEscape with a value greater than the number of capturing groups
            // is treated as either a LegacyOctalEscapeSequence or an IdentityEscape in Annex B,
            // an error is nevertheless reported since it's most likely a mistake.
            if value > self.number_of_capturing_groups {
                if self.number_of_capturing_groups > 0 {
                    let count = self.number_of_capturing_groups.to_string();
                    self.error(
                        diagnostics::This_backreference_refers_to_a_group_that_does_not_exist_There_are_only_0_capturing_groups_in_this_regular_expression,
                        pos,
                        end - pos,
                        vec![count],
                    );
                } else {
                    self.error(
                        diagnostics::This_backreference_refers_to_a_group_that_does_not_exist_There_are_no_capturing_groups_in_this_regular_expression,
                        pos,
                        end - pos,
                        Vec::new(),
                    );
                }
            }
        }
    }
}

fn compare_decimal_strings(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a = a.trim_start_matches('0');
    let mut b = b.trim_start_matches('0');
    if a.is_empty() {
        a = "0";
    }
    if b.is_empty() {
        b = "0";
    }
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    a.cmp(b)
}

/// Section 6.1.4
fn is_word_character(ch: i32) -> bool {
    chars::is_ascii_letter(ch) || chars::is_digit(ch) || ch == '_' as i32
}

// Table 66: Non-binary Unicode property aliases and their canonical property names
// https://tc39.es/ecma262/#table-nonbinary-unicode-properties
const NON_BINARY_UNICODE_PROPERTIES: &[(&str, &str)] = &[
    ("General_Category", "General_Category"),
    ("gc", "General_Category"),
    ("Script", "Script"),
    ("sc", "Script"),
    ("Script_Extensions", "Script_Extensions"),
    ("scx", "Script_Extensions"),
];

/// `nonBinaryUnicodeProperties[name]`: `""` when absent.
fn non_binary_unicode_property(name: &str) -> &'static str {
    NON_BINARY_UNICODE_PROPERTIES.iter().find(|&&(alias, _)| alias == name).map_or("", |&(_, canonical)| canonical)
}

// Table 67: Binary Unicode property aliases and their canonical property names
// https://tc39.es/ecma262/#table-binary-unicode-properties
const BINARY_UNICODE_PROPERTIES: &[&str] = &[
    "ASCII", "ASCII_Hex_Digit", "AHex", "Alphabetic", "Alpha", "Any", "Assigned",
    "Bidi_Control", "Bidi_C", "Bidi_Mirrored", "Bidi_M",
    "Case_Ignorable", "CI", "Cased",
    "Changes_When_Casefolded", "CWCF", "Changes_When_Casemapped", "CWCM",
    "Changes_When_Lowercased", "CWL", "Changes_When_NFKC_Casefolded", "CWKCF",
    "Changes_When_Titlecased", "CWT", "Changes_When_Uppercased", "CWU",
    "Dash", "Default_Ignorable_Code_Point", "DI", "Deprecated", "Dep",
    "Diacritic", "Dia",
    "Emoji", "Emoji_Component", "EComp", "Emoji_Modifier", "EMod",
    "Emoji_Modifier_Base", "EBase", "Emoji_Presentation", "EPres",
    "Extended_Pictographic", "ExtPict", "Extender", "Ext",
    "Grapheme_Base", "Gr_Base", "Grapheme_Extend", "Gr_Ext",
    "Hex_Digit", "Hex",
    "IDS_Binary_Operator", "IDSB", "IDS_Trinary_Operator", "IDST",
    "ID_Continue", "IDC", "ID_Start", "IDS",
    "Ideographic", "Ideo",
    "Join_Control", "Join_C",
    "Logical_Order_Exception", "LOE",
    "Lowercase", "Lower", "Math",
    "Noncharacter_Code_Point", "NChar",
    "Pattern_Syntax", "Pat_Syn", "Pattern_White_Space", "Pat_WS",
    "Quotation_Mark", "QMark",
    "Radical",
    "Regional_Indicator", "RI",
    "Sentence_Terminal", "STerm",
    "Soft_Dotted", "SD",
    "Terminal_Punctuation", "Term",
    "Unified_Ideograph", "UIdeo",
    "Uppercase", "Upper",
    "Variation_Selector", "VS",
    "White_Space", "space",
    "XID_Continue", "XIDC", "XID_Start", "XIDS",
];

// Table 68: Binary Unicode properties of strings
// https://tc39.es/ecma262/#table-binary-unicode-properties-of-strings
const BINARY_UNICODE_PROPERTIES_OF_STRINGS: &[&str] = &[
    "Basic_Emoji", "Emoji_Keycap_Sequence", "RGI_Emoji_Modifier_Sequence",
    "RGI_Emoji_Flag_Sequence", "RGI_Emoji_Tag_Sequence",
    "RGI_Emoji_ZWJ_Sequence", "RGI_Emoji",
];

// Unicode 15.1
const SCRIPT_VALUES: &[&str] = &[
    "Adlm", "Adlam", "Aghb", "Caucasian_Albanian", "Ahom", "Arab", "Arabic",
    "Armi", "Imperial_Aramaic", "Armn", "Armenian", "Avst", "Avestan",
    "Bali", "Balinese", "Bamu", "Bamum", "Bass", "Bassa_Vah", "Batk", "Batak",
    "Beng", "Bengali", "Bhks", "Bhaiksuki", "Bopo", "Bopomofo", "Brah", "Brahmi",
    "Brai", "Braille", "Bugi", "Buginese", "Buhd", "Buhid",
    "Cakm", "Chakma", "Cans", "Canadian_Aboriginal", "Cari", "Carian",
    "Cham", "Cher", "Cherokee", "Chrs", "Chorasmian",
    "Copt", "Coptic", "Qaac", "Cpmn", "Cypro_Minoan", "Cprt", "Cypriot",
    "Cyrl", "Cyrillic",
    "Deva", "Devanagari", "Diak", "Dives_Akuru", "Dogr", "Dogra",
    "Dsrt", "Deseret", "Dupl", "Duployan",
    "Egyp", "Egyptian_Hieroglyphs", "Elba", "Elbasan", "Elym", "Elymaic",
    "Ethi", "Ethiopic",
    "Geor", "Georgian", "Glag", "Glagolitic",
    "Gong", "Gunjala_Gondi", "Gonm", "Masaram_Gondi",
    "Goth", "Gothic", "Gran", "Grantha", "Grek", "Greek",
    "Gujr", "Gujarati", "Guru", "Gurmukhi",
    "Hang", "Hangul", "Hani", "Han", "Hano", "Hanunoo",
    "Hatr", "Hatran", "Hebr", "Hebrew",
    "Hira", "Hiragana", "Hluw", "Anatolian_Hieroglyphs",
    "Hmng", "Pahawh_Hmong", "Hmnp", "Nyiakeng_Puachue_Hmong",
    "Hrkt", "Katakana_Or_Hiragana",
    "Hung", "Old_Hungarian",
    "Ital", "Old_Italic",
    "Java", "Javanese",
    "Kali", "Kayah_Li", "Kana", "Katakana", "Kawi",
    "Khar", "Kharoshthi", "Khmr", "Khmer", "Khoj", "Khojki",
    "Kits", "Khitan_Small_Script", "Knda", "Kannada", "Kthi", "Kaithi",
    "Lana", "Tai_Tham", "Laoo", "Lao", "Latn", "Latin",
    "Lepc", "Lepcha", "Limb", "Limbu",
    "Lina", "Linear_A", "Linb", "Linear_B", "Lisu",
    "Lyci", "Lycian", "Lydi", "Lydian",
    "Mahj", "Mahajani", "Maka", "Makasar",
    "Mand", "Mandaic", "Mani", "Manichaean", "Marc", "Marchen",
    "Medf", "Medefaidrin", "Mend", "Mende_Kikakui",
    "Merc", "Meroitic_Cursive", "Mero", "Meroitic_Hieroglyphs",
    "Mlym", "Malayalam", "Modi", "Mong", "Mongolian",
    "Mroo", "Mro", "Mtei", "Meetei_Mayek", "Mult", "Multani",
    "Mymr", "Myanmar",
    "Nagm", "Nag_Mundari", "Nand", "Nandinagari",
    "Narb", "Old_North_Arabian", "Nbat", "Nabataean",
    "Newa", "Nkoo", "Nko", "Nshu", "Nushu",
    "Ogam", "Ogham", "Olck", "Ol_Chiki",
    "Orkh", "Old_Turkic", "Orya", "Oriya",
    "Osge", "Osage", "Osma", "Osmanya", "Ougr", "Old_Uyghur",
    "Palm", "Palmyrene", "Pauc", "Pau_Cin_Hau",
    "Perm", "Old_Permic", "Phag", "Phags_Pa",
    "Phli", "Inscriptional_Pahlavi", "Phlp", "Psalter_Pahlavi",
    "Phnx", "Phoenician", "Plrd", "Miao",
    "Prti", "Inscriptional_Parthian",
    "Rjng", "Rejang", "Rohg", "Hanifi_Rohingya",
    "Runr", "Runic",
    "Samr", "Samaritan", "Sarb", "Old_South_Arabian",
    "Saur", "Saurashtra", "Sgnw", "SignWriting",
    "Shaw", "Shavian", "Shrd", "Sharada",
    "Sidd", "Siddham", "Sind", "Khudawadi", "Sinh", "Sinhala",
    "Sogd", "Sogdian", "Sogo", "Old_Sogdian",
    "Sora", "Sora_Sompeng", "Soyo", "Soyombo",
    "Sund", "Sundanese", "Sylo", "Syloti_Nagri", "Syrc", "Syriac",
    "Tagb", "Tagbanwa", "Takr", "Takri",
    "Tale", "Tai_Le", "Talu", "New_Tai_Lue",
    "Taml", "Tamil", "Tang", "Tangut", "Tavt", "Tai_Viet",
    "Telu", "Telugu", "Tfng", "Tifinagh",
    "Tglg", "Tagalog", "Thaa", "Thaana", "Thai", "Tibt", "Tibetan",
    "Tirh", "Tirhuta", "Tnsa", "Tangsa", "Toto",
    "Ugar", "Ugaritic",
    "Vaii", "Vai", "Vith", "Vithkuqi",
    "Wara", "Warang_Citi", "Wcho", "Wancho",
    "Xpeo", "Old_Persian", "Xsux", "Cuneiform",
    "Yezi", "Yezidi", "Yiii", "Yi",
    "Zanb", "Zanabazar_Square",
    "Zinh", "Inherited", "Qaai",
    "Zyyy", "Common",
    "Zzzz", "Unknown",
];

const GENERAL_CATEGORY_VALUES: &[&str] = &[
    "C", "Other", "Cc", "Control", "cntrl", "Cf", "Format", "Cn", "Unassigned",
    "Co", "Private_Use", "Cs", "Surrogate",
    "L", "Letter", "LC", "Cased_Letter", "Ll", "Lowercase_Letter", "Lm", "Modifier_Letter",
    "Lo", "Other_Letter", "Lt", "Titlecase_Letter", "Lu", "Uppercase_Letter",
    "M", "Mark", "Combining_Mark", "Mc", "Spacing_Mark", "Me", "Enclosing_Mark",
    "Mn", "Nonspacing_Mark",
    "N", "Number", "Nd", "Decimal_Number", "digit", "Nl", "Letter_Number", "No", "Other_Number",
    "P", "Punctuation", "punct", "Pc", "Connector_Punctuation", "Pd", "Dash_Punctuation",
    "Pe", "Close_Punctuation", "Pf", "Final_Punctuation", "Pi", "Initial_Punctuation",
    "Po", "Other_Punctuation", "Ps", "Open_Punctuation",
    "S", "Symbol", "Sc", "Currency_Symbol", "Sk", "Modifier_Symbol",
    "Sm", "Math_Symbol", "So", "Other_Symbol",
    "Z", "Separator", "Zl", "Line_Separator", "Zp", "Paragraph_Separator",
    "Zs", "Space_Separator",
];

/// `valuesOfNonBinaryUnicodeProperties[name]`.
fn values_of_non_binary_unicode_properties(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "General_Category" => Some(GENERAL_CATEGORY_VALUES),
        "Script" => Some(SCRIPT_VALUES),
        // The Script_Extensions property of a character contains one or more Script values.
        // See https://www.unicode.org/reports/tr24/#Script_Extensions
        // Here, since each Unicode property value expression only allows a single value,
        // its values can be considered the same as those of the Script property.
        "Script_Extensions" => Some(SCRIPT_VALUES),
        _ => None,
    }
}
