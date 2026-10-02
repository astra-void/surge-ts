// @filename: legacy.d.ts
// tsc reports using such a binding as a value as TS1361, never as a UMD
// reference, so the name must read as bound here.
export declare function greet(name: string): string;
export as namespace Legacy;
// @filename: other.d.ts
declare const value: number;
export default value;
// @filename: consumer.ts
import type Legacy from "./other";
export type Q = Legacy;
