// @filename: index.ts
export * from "./missing";
// @filename: app.ts
import { User } from "./index";
let value = User;
