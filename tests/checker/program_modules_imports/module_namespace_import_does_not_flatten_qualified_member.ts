// @filename: core.ts
// The flattened `ns.Member` spelling is not a real name for a namespace
// member; resolving it would hide a genuine error behind an open type.
export namespace Inner { export interface Leaf { depth: number } }
// @filename: index.ts
import * as ns from "./core";
declare const leaf: ns.Leaf;
let value = leaf.depth;
