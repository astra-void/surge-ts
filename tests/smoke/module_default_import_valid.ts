// @filename: user.ts
export default function getName(): string { return "Ada"; }
// @filename: index.ts
import getName from "./user";
let name: string = getName();
