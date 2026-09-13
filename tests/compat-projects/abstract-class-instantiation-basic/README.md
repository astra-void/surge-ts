# abstract-class-instantiation-basic

`new AbstractClass()` is `TS2511`, underlined across the whole `new`
expression. The class's abstractness rides on its instance-side declaration, so
a generic abstract class and an ambient one are caught the same way, and the
expression still types as the instance — tsc reports the instantiation without
cascading.
