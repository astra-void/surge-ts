# relation-target-boolean-nullable-basic

tsc's `isRelatedTo` relates a definitely non-nullable source to a union of
`null`/`undefined` and one other member as that member alone, so the message
names `number`, not `number | undefined`. The union has to hold exactly two
or three members for that, and tsc's `boolean` is the union `false | true`:
`boolean | undefined` has three members with only one nullable, so the whole
target is named. surge counted `boolean` as one member and named `boolean`.
