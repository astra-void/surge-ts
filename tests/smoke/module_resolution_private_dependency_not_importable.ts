// @filename: a.ts
interface InternalUser { name: string; }
export type Box = { user: InternalUser };
// @filename: b.ts
import { Box } from "./a";
let user: InternalUser = { name: "Ada" };
