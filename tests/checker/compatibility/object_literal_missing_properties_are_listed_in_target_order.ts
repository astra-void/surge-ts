// Two missing properties are TS2739 with both named, in the target's own
// order; one missing property would be TS2741 naming it.
// @surge-compare: messages
let user: { name: string; alpha: number } = {};