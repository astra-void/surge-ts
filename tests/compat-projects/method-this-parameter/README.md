# method-this-parameter

A method's own `this` parameter is what `this` means in its body, for an
instance method and a static one alike: `explicit` reads `x` off `Other`,
`make` constructs through `typeof Other`, and `wrong` reading `y` — a member
of `Host`, not of `Other` — is TS2339.
