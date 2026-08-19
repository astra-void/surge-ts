import * as viaAlias from "aliased-namespace-first";
import * as direct from "namespace-first";
import * as valueFirst from "value-first";
import fnMerge = require("function-namespace-merge");

export const a: string = viaAlias.resolve("x");
export const b: string = direct.resolve("x");
export const c: string = valueFirst.resolve("x");
export const d: string = fnMerge.version;
fnMerge();

export const wrong: number = direct.resolve("x");
