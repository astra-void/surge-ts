# no-unchecked-indexed-access-basic

Under `noUncheckedIndexedAccess` an array element read, a non-literal tuple index,
and a member reached only through a string index signature read as
`T | undefined`; a tuple element at a literal index and a declared member stay
exact. Destructuring an array (`const [head] = posts`) is such a read too. A
guard on the binding (`if (!head) return`), iteration, and assignment targets
are unaffected. surge did not model the option.
