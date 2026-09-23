# tuple-union-indexed-access-type

tsc distributes an indexed access type over a union receiver: a union of
tuples read at a numeric index is each member's element there, `undefined` for
a member too short for it (a rest element answers every later index), and a
missing property (TS2339) only when every member is too short. Reading it at
`number` is the union of all elements.
