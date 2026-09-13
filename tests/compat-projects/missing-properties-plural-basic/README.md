# missing-properties-plural-basic

A missing required property is only `TS2741` when exactly one is missing. tsc
names every missing property and picks the code by how many there are: two to
five are listed in full as `TS2739`, six or more list the first four as
`TS2740` with the rest counted. surge reported the first missing property as
`TS2741` in every case.
