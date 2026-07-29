import * as core from "./core/index.js";
import { util } from "./core/index.js";
import * as direct from "./direct.js";

declare const schema: core.Schema<string, number>;
declare const some: core.SomeType;
declare const viaStarAs: core.util.TupleItems;
declare const viaNamed: util.TupleItems;
declare const viaStarAsDeep: core.util.Deep.Nested;
declare const member: direct.Outer.Member;
declare const leaf: direct.Outer.Inner.Leaf;

export const a1: string = schema._output;
export const a2: number = schema._input;
export const a3: string = some.marker;

export const b1: number = viaStarAs.length;
export const b2: number = viaNamed.length;
export const b3: number = viaStarAsDeep.depth;

export const c1: string = member.value;
export const c2: number = leaf.leaf;
