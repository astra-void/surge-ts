# weak-type-relations

tsc's weak-type check (`isWeakType` / `hasCommonProperties`): a primitive or
object with no property in common with an all-optional target is TS2559, a
function whose call result would fit is TS2560, and a class implementing a
weak interface without a shared member is TS2559 on the class. An empty class
has no properties, so the check does not run for it. Implementing a class
without its members is TS2720.
