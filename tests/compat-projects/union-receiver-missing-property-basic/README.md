# union-receiver-missing-property-basic

A property read on a union is looked up on the union: tsc names the whole
union in TS2339, and offers a TS2551 spelling suggestion only among the
properties every member has. surge reported the first member lacking the
property and suggested that member's own names, so `u.property1` on
`{ property1 } | { property2 }` came out as a TS2551 for `property2` — a
different code. The call form reports the missing property before
callability, with the same suggestion rule.
