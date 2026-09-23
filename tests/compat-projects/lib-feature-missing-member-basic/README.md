# lib-feature-missing-member-basic

A member the configured lib lacks but a later lib declares is TS2550 ("Do you
need to change your target library?"), not TS2339: tsc's
`getSuggestedLibForNonExistentProperty` looks the member up in the feature map
under the receiver's apparent type — a typed array's `at`, an `Error`'s
`cause` (es2022), as well as an array's or a string's. A member no lib
declares is still TS2339.

Paired with `lib-feature-member-present`, which targets ES2022.
