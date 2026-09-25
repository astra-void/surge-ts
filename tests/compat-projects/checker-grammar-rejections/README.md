# checker-grammar-rejections

Grammar errors tsc's parser accepts and its checker rejects, each reported
without stopping the file: an implementation or a statement in an ambient
context (TS1183, TS1036), an accessor with no body (TS1005 `'{' expected`,
which a `declare` accessor does not get — its modifier is rejected first), a
signature overload that disagrees on `?` (TS2386), an index signature with
the wrong number of parameters (TS1096) or no annotation (TS1021, reported
only for a key tsc accepts), a line break before `=>` (TS1200), a constant
enum shift of 32 or more (TS6807), and `accessor` before an import or export
(TS1275).
