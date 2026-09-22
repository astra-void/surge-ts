# write-target-kinds-basic

tsc's `checkIdentifier` rejects a write by what the name declares: an enum
(TS2628), a class (TS2629), a function (TS2630), a namespace (TS2631), and only
otherwise a `const` (TS2588). surge reported the first two as TS2588 and the
namespace as an unresolved name.
