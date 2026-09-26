# constructor-return-basic

A value a class constructor returns must be assignable to the class instance type (`checkReturnStatement`): a mismatch is TS2322 at the `return` statement plus TS2409 there. The whole value is related, so a returned conditional is one mismatch, not one per branch. A bare `return`, `return this`, a value of the class type, a value an empty class accepts, and the returns of functions nested in the constructor report nothing.
