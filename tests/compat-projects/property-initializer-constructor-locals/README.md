# property-initializer-constructor-locals

tsc's name resolver remembers a non-static property declaration while it climbs
out of one (`propertyWithInvalidInitializer` in `binder/nameresolver.go`) when
the class's constructor implementation declares the name being resolved as a
local: a parameter, a top-level declaration of its body, or a `var` the body
hoists. Unless class fields are emitted as standard fields
(`useDefineForClassFields: false` here), `checkAndReportErrorForInvalidInitializer`
then reports the reference instead of resolving it: first as a missing `this.`
or `C.` prefix (TS2663/TS2662) when the class has such a member, otherwise as
TS2844 inside the property's type annotation and TS2301 anywhere else in it.

A static property, a name the constructor declares only in a nested block, and
a name an inner function binds are ordinary lookups (TS2304, or nothing).
