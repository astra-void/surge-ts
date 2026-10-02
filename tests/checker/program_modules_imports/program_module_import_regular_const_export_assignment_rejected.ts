// @filename: user.ts
export const value: string = "Ada";
// @filename: index.ts
import { value } from "./user";
value = "Grace";
