# export-equals-entity-meanings

tsc's `getTargetOfImportEqualsDeclaration` → `resolveExternalModuleSymbol`:
`import x = require("m")` *is* the entity `m`'s `export =` names, with every
meaning it has — value, type and namespace members. surge bound only the value,
so a class assigned with `export =` could not be used as a type through the
import (a false TS2749).

The export side follows `bindExportAssignment` / `getTargetOfAliasLikeExpression`:

- an entity name — `export = ClassB`, `export = A.B`, `export = foo.bar.X`, a
  global namespace included — is an alias carrying the entity's meanings;
- any other expression (`export = "foo".length`, `export = (Q)`) is a property
  whose only meaning is the expression's value, so the import is a value
  and nothing else (TS2749 as a type).

`export default` follows the same rule: `export default D.E` exports the class
type too, `export default (P.F)` only its value.

The class merged with a namespace inside a namespace (`A.B`, `D.E`) keeps its
constructor through the merge; surge models such a nested class's value
permissively, so members missing from it are not asserted here.

A named import from an `export =` module (`getExternalModuleMember`) reads the
value off the entity's type and the types off the entity's namespace members,
which is how `import { X } from "foobar"` reaches `foo.bar.X`.
