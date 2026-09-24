import { hoisted, TypesOnly, Values } from "./a";
const n: number = hoisted + Values.v;
const s: TypesOnly.Shape = { s: "" };
export { TypesOnly as Reexported, n, s };
