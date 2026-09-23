# numeric-enum-reverse-mapping

An enum with a numeric member (or no members) has a reverse mapping: its object
type carries a `number` index signature of type `string` (tsc
`enumNumberIndexInfo`), so `Direction[0]` and `Mixed[1]` are `string`. An enum
of only string members has no index signature, and indexing it by `string` is
TS7053.
