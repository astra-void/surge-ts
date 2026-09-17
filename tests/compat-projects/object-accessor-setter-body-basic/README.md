# object-accessor-setter-body-basic

An object literal's `set` accessor paired with a `get` of the same name was
dropped during lowering (the getter decides the property's type), so its
body was never checked. It is now kept on the getter and checked, its
untyped parameter taking the getter's annotation.
