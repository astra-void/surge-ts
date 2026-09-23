# protected-access-through-instance

tsc's `checkPropertyAccessibilityAtLocation`: a protected instance member is
reached from the innermost enclosing class that derives from its declaring
class (or from the class a `this` parameter names), and only through an
instance of that class or a class deriving from it. `base.x` and
`sibling.x` inside `Derived1` are TS2446, as is `base.x` in a static method;
`own.x`, `deeper.x`, `this.x` and a `super` access are fine.
