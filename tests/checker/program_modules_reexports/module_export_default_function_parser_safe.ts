// @filename: user.ts
export default function makeThing() { return "Ada"; }
// @filename: index.ts
import value from "./user";
let name: string = value();
