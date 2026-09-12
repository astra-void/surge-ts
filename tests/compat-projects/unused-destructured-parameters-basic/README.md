# unused-destructured-parameters-basic

tsc groups unused destructured parameter bindings per pattern: a pattern whose
every element is unused collapses to one `TS6198` on the pattern, otherwise each
unused binding is a `TS6133` naming the local. `_`-prefixing exempts an object
binding only when it renames a property (`{ a: _a }`); shorthand `{ _x }` still
reports. An array element or array rest is exempt by prefix; an object rest is
not, and the named siblings of an object rest are never reported because they
keep properties *out* of it. surge previously skipped patterns entirely.
