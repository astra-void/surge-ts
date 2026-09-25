# class-member-duplicates

Properties, accessors and parameter properties of one name merge into one
symbol, so tsc's checker, not its binder, finds their duplicates
(`checkObjectTypeForDuplicateDeclarations`): a second property, or a property
beside an accessor, is TS2300 at every member of that name — a get/set pair
and a static member beside an instance one are not. The merged declarations
must also agree on their modifiers (`areDeclarationFlagsIdentical`): the first
declaration and each one differing from it are TS2687.
