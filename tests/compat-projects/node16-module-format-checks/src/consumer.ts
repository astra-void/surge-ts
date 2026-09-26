import { value } from "./esm.mjs";
import type { Shape } from "./esm.mjs";
import esm = require("./esm.mjs");

await Promise.resolve(value);
const url = import.meta.url;

export const shape: Shape = { size: value };
export { esm, url };
