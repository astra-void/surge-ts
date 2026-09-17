# this-write-target-anchor-basic

`this.p = value` is an ordinary assignment to tsc: a mismatch is reported on
the target `this.p`, and an object literal value elaborates into its members
(TS2322 on the member, TS2741 on the target). surge evaluated the value with no
contextual type and reported the whole value where it was written.
