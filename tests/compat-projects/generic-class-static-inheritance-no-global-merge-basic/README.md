# generic-class-static-inheritance-no-global-merge-basic

A class that inherits its base's static side must not pick up an *ambient
global* of the same name.

A generic class models its value side as `any` and contributes no value symbol,
so the derived lookup missed the module's own table. A parent-traversing lookup
then found the DOM's `MutationObserver`, merged the base's statics into that,
and published it as the module's export — every `new MutationObserver(client,
options)` met the DOM's one-parameter constructor (51 false `TS2554` across the
tanstack-query aggregate). The lookup is own-table now.

The fixture asserts an *absence*: both compilers report nothing. The reads off
the instance are deliberately not asserted — a generic class still models its
value side as `any`, so surge sees no type there to check, which is a separate
gap.
