// @filename: a.ts
type User = { name: string };
export { User };
// @filename: b.ts
import { User } from "./a";
let user: User = { name: "Ada" };
