# this-before-super-basic

In a derived class constructor, `this` (TS17009) and `super.x` (TS17011)
read before `super()` has run are errors, including inside `super()`'s own
arguments. A reference in a nested arrow or function runs later and is not
one. surge reported neither.
