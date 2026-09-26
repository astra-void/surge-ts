# declaration-file-ambient-grammar

Every node of a declaration file is ambient, and tsc checks a declaration file
(unless `skipLibCheck`) with the grammar that follows from it: `declare` on a
declaration in a namespace or module body is TS1038, a namespace declared
with the `module` keyword is TS1540, a generator is TS1221, a function body is
TS1183, and an export assignment of anything but an entity name is TS2714.
