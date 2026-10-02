// A written out-of-range index on a *concrete* tuple is the diagnostic tsc does
// emit, and it still does.
type Pair = [string];
export type Second = Pair[1];
