# protected-access-this-parameter

tsc's `getEnclosingClassFromThisParameter`: outside every class that derives
from a protected member's declaring class, the class a function's `this`
parameter names — through a type parameter's constraint — reaches the
member, a static one excepted. A method's own `this` parameter counts; a
nested function declares its own `this` and so does not inherit one, while an
arrow does.
