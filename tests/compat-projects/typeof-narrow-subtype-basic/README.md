# typeof-narrow-subtype-basic

`getNarrowedType` for a `typeof` test on a non-union receiver: the tag's
own type stands in when it is a subtype of the receiver (`let wide: {}`
tested for `"number"` is a `number`), the receiver stands when it is the
narrower of the two, and a tag it can never report leaves the branch
unreachable. surge kept the declared type in every one of these.
