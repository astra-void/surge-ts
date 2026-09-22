# relation-rules-basic

Three rules of tsc's relater that surge was missing:

- `reportRelationError` drops its own head (TS2322/TS2345) when the failure
  beneath it is a missing property of the same pair, so a source lacking a
  required member reports TS2741 (one), TS2739 (up to five) or TS2740. A
  function source, the global `Object`, the `object` keyword, an
  intersection target and two instantiations of one generic keep the head.
- `propertyRelatedTo`: a `private` member relates only to itself, a
  `protected` one only to a restricted member.
- `typeRelatedToDiscriminatedType`: an object whose discriminants (`boolean`,
  literal unions, `undefined`) can each select target members relates to the
  union when every combination does.
