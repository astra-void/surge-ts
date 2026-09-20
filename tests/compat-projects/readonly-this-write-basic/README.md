# readonly-this-write-basic

tsc's `isAssignmentToReadonlyEntity`: `this.x = …` may initialize a `readonly`
member only when the control-flow container is the constructor of the class
that declares it — as a property or a parameter property. A method, an
accessor, a function or arrow nested in that constructor, and a derived
class's constructor all report TS2540, and they report it *instead of*
checking the value. A `static readonly` member and a getter-only static are
read-only through the class value. surge ran no readonly check on a `this`
write and recorded no `readonly` on the static side.
