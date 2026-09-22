# union-target-missing-property-basic

Two rules about where and how a literal's *whole-value* failure is reported.

Against a union target tsc keeps the outer assignability code for a missing
required property — TS2322, or TS2345 for an argument — and names the union;
the "Property 'y' is missing…" text is only its elaboration. surge checked the
literal against the member it picked and reported that member's TS2741. A
union that is one object plus `undefined` is that object and keeps TS2741.

A nested literal that fails as a whole is reported on the *name* of the
property holding it, where `elaborateObjectLiteral` stops descending; surge
reported it on the nested literal. An array element is still reported on the
element.
