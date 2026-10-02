// Two instantiations of the same degraded declaration must keep their own
// arguments. A memo entry is keyed on a *display-inclusive* fingerprint of the
// substitution precisely so `Box<string>` cannot be served `Box<number>`'s
// expansion (the canonical-store display-substitution class, which shows up as
// message drift rather than as a wrong type).
// @surge-compare: messages
interface Def<T> { tag: Missing.Tag; inner: T }
interface Box<T> { def: Def<T>; value: T }
declare const a: Box<string>;
declare const b: Box<number>;
export const x = a.nope;
export const y = b.nope;
