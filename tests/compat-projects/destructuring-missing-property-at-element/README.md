# destructuring-missing-property-at-element

A destructuring element reads its source through tsc's `getIndexedAccessType`
at the element's name, with no element access expression: a property the
source lacks is TS2339 on that name, whatever `noImplicitAny` says, and never
the element-access TS7053. A defaulted element of an object-literal source reads
`undefined` instead (`AccessFlags.AllowMissing`) — only while the source is the
literal itself; a variable holding one has the widened type and still reports.
An array literal initializer is typed as a tuple from the pattern, so nested
patterns read their own element.
