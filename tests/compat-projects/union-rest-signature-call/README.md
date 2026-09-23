# union-rest-signature-call

Calling a union of functions uses the one signature tsc combines for it
(`combineUnionOrIntersectionParameters`): each position's parameter is the
intersection of what every member takes there, a rest parameter answering
every position from its own on with its element. So `{ x, y }` arguments fit
`{ x } & { y }`, and an argument missing either property does not.
