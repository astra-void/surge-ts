# tsc-regular-expressions

The checker's grammar check of regular expression literals
(`checkGrammarRegularExpressionLiteral`, typescript-go's `scanner/regexp.go`):
flags, and flags the `es2015` target lacks; groups, named groups and their
target; quantifiers; escapes with and without the Unicode flags; character
classes, ranges and Unicode Sets operators; `\p{…}` names and values; and
backreferences.
