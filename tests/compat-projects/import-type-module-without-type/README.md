# import-type-module-without-type

An import type with no qualifier names the module itself, which is a type only
when its `export =` is one. A class `export =` is; a namespace `export =` and
an ES module are not, and tsc reports TS1340 on the import type, suggesting
`typeof import(…)` (which reads the module as a value and is fine).
