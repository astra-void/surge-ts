# named-constraint-primitive-argument-basic

When an inferred type argument does not satisfy its constraint, tsc infers the
constraint instead (`getInferredType`), so `measure(5)` for
`T extends HasLength` checks `5` against `HasLength` and reports TS2345. surge
declined to judge a constraint that is a named reference — expanding one could
touch declarations the call never reads — so the call went unchecked.

A primitive candidate now peels the named constraint when its shape is fully
modelled: the argument check reads that shape anyway. Primitives and objects
that do satisfy it (`"text"`, an array, `{ length: 3 }`) are pinned as clean.
