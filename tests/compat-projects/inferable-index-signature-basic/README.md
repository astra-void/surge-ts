# inferable-index-signature-basic

`isObjectTypeWithInferableIndex`: only an object or type literal has an
implicit index signature, so an interface or class instance answers a target
index signature with one it declares or inherits, or not at all. A derived
interface inherits its base's *number* index signature (only the string one
was carried), including from an array base — without it every numeric read
was a false TS7053 and the derived type was not assignable to its own base.
An `any`-valued index signature does not supply a required property. An
intersection of object types names its missing members as any object does;
one with a primitive operand keeps the plain head.
