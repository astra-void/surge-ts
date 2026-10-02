// @filename: data.json
{ "version": "1.2.3" }
// @filename: example.ts
import { version } from "./data.json";
export const v: number = version;
