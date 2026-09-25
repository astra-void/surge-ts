# namespace-import-member-writes

Every member of a namespace import (`import * as ns`) is read-only
(`isAssignmentToReadonlyEntity`), whatever the export's own declaration says,
so a write to one is TS2540 — an assignment, an update (`++`/`--`) or a
`for…of` head alike. The member is resolved first: a name the module
does not export is TS2339 on the write, as on a read, and never TS2540. A
parameter that shadows the import is an ordinary object.
