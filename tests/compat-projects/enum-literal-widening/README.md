# enum-literal-widening

An enum member read off its enum is a fresh literal type (tsc's
`getDeclaredTypeOfEnum` declares every member fresh), so a mutable location
widens it to the enum (`getWidenedLiteralType` → `getBaseTypeOfEnumLikeType`):
a `let`, an object literal property, an array literal element and a class
property initializer all hold the enum, while the declaration itself still
narrows to the member (`getAssignmentReducedType`), which a comparison sees.
