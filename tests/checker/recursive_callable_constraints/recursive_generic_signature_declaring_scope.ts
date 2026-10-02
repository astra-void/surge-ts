// @strict: true
// @filename: dependency.ts
export interface Bound<T> { value: T; }
export type Direct = <T extends Bound<T>>(value: T) => void;
export type Nested = <U extends Bound<Bound<U>>>(value: U) => void;

// @filename: consumer.ts
import type { Direct, Nested } from "./dependency";
interface Bound<T> { unrelated: T; }
declare let direct: Direct;
declare let nested: Nested;
direct = nested;
nested = direct;
