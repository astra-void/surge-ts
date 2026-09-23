# enum-computed-member-name-basic

`computeEnumMemberValue`: an enum member named by a computed expression that
is not a string or no-substitution template literal is TS1164 over the whole
`[…]` name; literal computed names are fine. Every such member is reported and
the rest of the file is still checked; `other.ts` shows the remaining files are
checked too.
