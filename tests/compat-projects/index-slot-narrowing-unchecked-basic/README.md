# index-slot-narrowing-unchecked-basic

A name read through a string index signature is a narrowable reference. Under
`noUncheckedIndexedAccess` the unguarded read is `T | undefined`; a guard that
proves the slot present (or a write that fills it) leaves `T`, even when `T`
itself does not change. The slot still *comes from* the index signature, so
`noPropertyAccessFromIndexSignature` keeps reporting the dotted spelling —
reads and writes alike — inside the narrowed branch and after it.
