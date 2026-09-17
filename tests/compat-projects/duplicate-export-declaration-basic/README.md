# duplicate-export-declaration-basic

An exported name carried by both a local `export`ed declaration and an
`export { … }` specifier is two declarations of one export. tsc reports the
pair on every declaration as TS2323 unless the local one legally merges
(interface, type alias, namespace, enum), and separately reports the
specifier as TS2484 when its target overlaps the local declaration in
meaning. surge reported neither.
