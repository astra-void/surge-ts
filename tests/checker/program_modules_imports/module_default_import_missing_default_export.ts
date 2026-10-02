// @filename: user.ts
export const getName: string = "Ada";
// @filename: index.ts
import getName from "./user";
let name = getName;
