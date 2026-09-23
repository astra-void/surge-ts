# enum-computed-member-name-basic

`computeEnumMemberValue`: an enum member named by a computed expression that
is not a string or template literal is TS1164, reported on the bracketed name.
Literal computed names are allowed.

surge takes TS1164 from oxc, which treats it as a fatal parse error: only the
first one in a file is reported and the rest of that file goes unchecked. Each
file here therefore holds a single TS1164; `other.ts` shows the remaining files
are still checked.
