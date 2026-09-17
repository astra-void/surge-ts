# branded-primitive-assignability-basic

An intersection is assignable to a target when some constituent is
(`someTypeRelatedToType`), so a branded primitive — `"lazy" & { __brand: "lazy" }`
or `string & { __tag: unique symbol }` — is assignable to its primitive. surge
merges such an intersection to its object side, which dropped the primitive, so
every use of a branded value where the primitive was expected was a false
TS2322. The merge now records the primitive operand, and assignability relates
through it. The brand is still required in the other direction, a branded value
is still not a `number`, and an intersection of objects is still not a `string`.
