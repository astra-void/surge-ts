// A receiver with neither the member nor an index signature still reports.
// @noImplicitAny: true
interface Fixed { a: number }
declare const fixed: Fixed;
export const a = fixed[0];
