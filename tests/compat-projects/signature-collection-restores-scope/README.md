# signature-collection-restores-scope

Hoisted function signatures are collected before the class after them is
built. The collection must leave the file's scope as it found it: a static
method's unannotated parameter default read through an import (`import x =
require()` or `import * as`) resolves, and only a name declared nowhere is
TS2304.
