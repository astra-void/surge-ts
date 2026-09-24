//! `ast.NodeFlags` and `ast.TokenFlags`, with typescript-go's names.

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

macro_rules! flags {
    ($name:ident($repr:ty) { $($flag:ident = $value:expr,)* }) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
        pub struct $name(pub $repr);
        #[allow(non_upper_case_globals, dead_code)]
        impl $name {
            $(pub const $flag: $name = $name($value);)*
            pub fn has(self, other: $name) -> bool { self.0 & other.0 != 0 }
            pub fn is_empty(self) -> bool { self.0 == 0 }
        }
        impl BitOr for $name { type Output = $name; fn bitor(self, o: $name) -> $name { $name(self.0 | o.0) } }
        impl BitAnd for $name { type Output = $name; fn bitand(self, o: $name) -> $name { $name(self.0 & o.0) } }
        impl BitOrAssign for $name { fn bitor_assign(&mut self, o: $name) { self.0 |= o.0 } }
        impl BitAndAssign for $name { fn bitand_assign(&mut self, o: $name) { self.0 &= o.0 } }
        impl Not for $name { type Output = $name; fn not(self) -> $name { $name(!self.0) } }
    };
}

flags!(NodeFlags(u32) {
    None = 0,
    Let = 1 << 0,
    Const = 1 << 1,
    Using = 1 << 2,
    Reparsed = 1 << 3,
    Synthesized = 1 << 4,
    OptionalChain = 1 << 5,
    ExportContext = 1 << 6,
    ContainsThis = 1 << 7,
    HasImplicitReturn = 1 << 8,
    HasExplicitReturn = 1 << 9,
    DisallowInContext = 1 << 10,
    YieldContext = 1 << 11,
    DecoratorContext = 1 << 12,
    AwaitContext = 1 << 13,
    DisallowConditionalTypesContext = 1 << 14,
    ThisNodeHasError = 1 << 15,
    JavaScriptFile = 1 << 16,
    ThisNodeOrAnySubNodesHasError = 1 << 17,
    HasAsyncFunctions = 1 << 18,
    PossiblyContainsDynamicImport = 1 << 19,
    PossiblyContainsImportMeta = 1 << 20,
    HasJSDoc = 1 << 21,
    JSDoc = 1 << 22,
    Ambient = 1 << 23,
    InWithStatement = 1 << 24,
    JsonFile = 1 << 25,
    PossiblyContainsDeprecatedTag = 1 << 26,
    Unreachable = 1 << 27,
    ReparserTransformedLiteral = 1 << 28,
    BlockScoped = (1 << 0) | (1 << 1) | (1 << 2),
    Constant = (1 << 1) | (1 << 2),
    AwaitUsing = (1 << 1) | (1 << 2),
    ContextFlags = (1 << 10) | (1 << 14) | (1 << 11) | (1 << 12) | (1 << 13) | (1 << 16) | (1 << 24) | (1 << 23),
    TypeExcludesFlags = (1 << 11) | (1 << 13),
    IdentifierHasExtendedUnicodeEscape = 1 << 7,
    NestedNamespace = 1 << 5,
});

flags!(TokenFlags(u32) {
    None = 0,
    PrecedingLineBreak = 1 << 0,
    PrecedingJSDocComment = 1 << 1,
    Unterminated = 1 << 2,
    ExtendedUnicodeEscape = 1 << 3,
    Scientific = 1 << 4,
    Octal = 1 << 5,
    HexSpecifier = 1 << 6,
    BinarySpecifier = 1 << 7,
    OctalSpecifier = 1 << 8,
    ContainsSeparator = 1 << 9,
    UnicodeEscape = 1 << 10,
    ContainsInvalidEscape = 1 << 11,
    HexEscape = 1 << 12,
    ContainsLeadingZero = 1 << 13,
    ContainsInvalidSeparator = 1 << 14,
    PrecedingJSDocLeadingAsterisks = 1 << 15,
    SingleQuote = 1 << 16,
    PrecedingJSDocWithDeprecated = 1 << 17,
    PrecedingJSDocWithSeeOrLink = 1 << 18,
    BinaryOrOctalSpecifier = (1 << 7) | (1 << 8),
    WithSpecifier = (1 << 6) | (1 << 7) | (1 << 8),
    StringLiteralFlags = (1 << 2) | (1 << 12) | (1 << 10) | (1 << 3) | (1 << 11) | (1 << 16),
    NumericLiteralFlags = (1 << 4) | (1 << 5) | (1 << 13) | (1 << 6) | (1 << 7) | (1 << 8) | (1 << 9) | (1 << 14),
    TemplateLiteralLikeFlags = (1 << 2) | (1 << 12) | (1 << 10) | (1 << 3) | (1 << 11),
    IsInvalid = (1 << 5) | (1 << 13) | (1 << 14) | (1 << 11),
});
