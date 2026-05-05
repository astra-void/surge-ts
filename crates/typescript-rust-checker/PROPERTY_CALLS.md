# Property Calls

Supported in this phase:

- `object.property(...)` where `object` is a simple identifier and `property` is a direct property name.
- Function-typed object, interface, and type-alias properties.
- Return-type propagation into declarations, assignments, returns, and supported conditionals.
- Contextual checking of object literal property values when the property value is a property call.
- Inferred object literal property values when the property value is a property call.
- Optional property calls and direct optional calls returning ReturnType | undefined. Nested optional chains work predictably.
- `any` receivers return `any` and skip argument checking.
- `unknown` receivers remain intentionally shallow and currently do not emit a property-call diagnostic.

Not supported in this phase:

- Chained property calls.
- Nested property access calls.
- Bracket notation.
- Method declarations or shorthand methods.
- Receiver or `this` binding semantics.
- Callable unions.
- Callback contextual typing.
- Function expressions or arrow functions.

Diagnostic behavior:

- Argument checking happens only after the callee property resolves to a callable property.
- Missing properties, non-callable properties, unresolved objects, and arity mismatches short-circuit before argument checking.
- Optional function properties widen to `fn | undefined` and are not callable without narrowing.
- Primitive and function receivers are not special-cased and still emit TS2339.
