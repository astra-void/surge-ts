# super-abstract-member-access

A `super` access reaches the base class's implementation, so a member whose
nearest declaration up the chain is `abstract` cannot be reached that way
(TS2513, `checkPropertyAccessibilityAtLocation`), whether it is called or
read. A concrete member further up is only reached when no abstract
declaration hides it.
