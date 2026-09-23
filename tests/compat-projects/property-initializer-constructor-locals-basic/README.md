# property-initializer-constructor-locals-basic

With `useDefineForClassFields: false` an instance property initializer is
emitted into the constructor, so reading a name the constructor declares is
TS2301. Static initializers, shadowing parameters and names the constructor
does not declare are not affected.
