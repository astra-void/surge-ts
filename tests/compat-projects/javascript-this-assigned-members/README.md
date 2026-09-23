# javascript-this-assigned-members

tsc declares a JavaScript class member from `this.x = v` in the class's own
methods (`bindThisPropertyAssignment`) and types it from what is assigned
(`getWidenedTypeForAssignmentDeclaration`): a member the constructor assigns
takes only the constructor's values (`count` is `number` though `load`
assigns a string), one only methods assign may still be missing (`label` is
`string | undefined`, TS2322), an empty array is `any[]`, one assigned only
`null` is `any`, and `this.pending = this.pending || []` declares `pending`
without typing it. A static method's `this` declares a static member.
