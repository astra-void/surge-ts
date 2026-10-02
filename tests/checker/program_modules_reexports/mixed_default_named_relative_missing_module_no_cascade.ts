// The module itself is unresolved (TS2307); both the default and named
// bindings fall back to unknown so usages must not cascade into TS2304.
import DefaultThing, { helper } from "./missing";
let count = helper();
let made = DefaultThing();
