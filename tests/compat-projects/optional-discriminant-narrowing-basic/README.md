# optional-discriminant-narrowing-basic

An optional discriminant (`tag?: "only"`) may be absent, so tsc keeps the
member on the `tag !== "only"` side: the property is `undefined` there, the
object is still itself. surge lost the object twice. Narrowing the property
left only `undefined`, which `remove_undefined` answers with the degradation
sentinel — the whole binding then read as unresolved and every later check on
it was silent. And the union filter treated the optional member as one whose
discriminant *is* the literal, dropping it from the complement.
