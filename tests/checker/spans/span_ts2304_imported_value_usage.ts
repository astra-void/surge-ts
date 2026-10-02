// @surge-compare: spans
// @filename: user.ts
export const value = 1;
// @filename: index.ts
import { value as localValue } from "./user"; localValue;
