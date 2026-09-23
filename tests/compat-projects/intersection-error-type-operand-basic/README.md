# intersection-error-type-operand-basic

tsc's `getIntersectionType` returns the error type when an operand is the
error type (`checker.go` 26443), the same way `any` absorbs an intersection.
A type imported from a module that does not resolve is that error type, so
`Missing & { label: string }` accepts any value: no excess property, no
mismatched member. surge dropped the error-type operand and kept the object
operand closed, which reported the object literals written against it.

Only the unresolved import is reported. `Known` has no error-type operand and
still reports its excess property.
