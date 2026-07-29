import * as core from "./core/index.js";
import * as direct from "./direct.js";

declare const viaStarAs: core.util.TupleItems;
declare const viaStarAsDeep: core.util.Deep.Nested;
declare const leaf: direct.Outer.Inner.Leaf;

export const wrongTuple: string = viaStarAs.length;
export const wrongDeep: string = viaStarAsDeep.depth;
export const wrongLeaf: string = leaf.leaf;
