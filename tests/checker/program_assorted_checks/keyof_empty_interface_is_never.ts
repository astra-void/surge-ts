// `keyof {}` is `never`, so the empty-interface escape hatch React uses for
// `Key`/`ReactNode` (`T[keyof T]` over a members-less interface) contributes
// nothing to its union instead of degrading it. A type surge could not model
// still yields the `unknown` sentinel rather than a closed `never`.
interface Escape {}
type Key = string | number | Escape[keyof Escape];
declare const k: Key;
export const s: string | number = k;
export const bad: string = k;
