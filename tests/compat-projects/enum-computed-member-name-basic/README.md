# enum-computed-member-name-basic

An enum member named by a computed expression is TS1164 over the whole
`[…]` name; a string or no-substitution template in brackets is a literal
name and fine. Every such member is reported and the rest of the file is
still checked.
