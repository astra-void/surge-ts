# unannotated-body-return-checked-basic

An unannotated function declaration returns what its body returns: tsc's
`getReturnTypeFromBody` unions the `return` expressions (literals widened), adds
`undefined` when the end is reachable, and answers `void` when nothing is
returned. surge read these returns with a sketch that gave up on any body with
branches, nested functions or local types, leaving the declaration's return at
the degradation sentinel.
