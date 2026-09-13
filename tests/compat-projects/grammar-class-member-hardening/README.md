# grammar-class-member-hardening

Everything one class body can say wrong about its own members, none of which
needed a type to see and none of which surge reported:

- a derived constructor with no `super` call (`TS2377`),
- two constructor implementations (`TS2392`),
- two implementations of one method (`TS2393`),
- two members declaring the same name where neither is an overload of the
  other (`TS2300`) — property/property, property/method, and the object-literal
  method pair at the bottom.

The negatives are the point of the rest: `CallsSuper` finds its call inside a
branch, `StaticAndInstance` is two different names, `Overloaded` is an overload
group with one implementation and a `get`/`set` pair, and
`OverloadedMember` is the interface half of the same rule — repeated *method*
signatures are overloads, a repeated property is a duplicate.
