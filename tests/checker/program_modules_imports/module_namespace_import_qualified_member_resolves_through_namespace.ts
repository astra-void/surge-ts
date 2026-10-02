// @filename: core.ts
// A namespace member reached through a namespace import keeps its namespace
// qualifier (`ns.Inner.Member`). Flattening it to `ns.Member` in the alias
// table left the real name unresolvable, so the type silently opened and the
// member error below was never reported.
export namespace Inner { export interface Leaf { depth: number } }
// @filename: index.ts
import * as ns from "./core";
declare const leaf: ns.Inner.Leaf;
let wrong: string = leaf.depth;
