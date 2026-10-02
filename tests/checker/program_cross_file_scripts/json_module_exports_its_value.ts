// @filename: data.json
{ "version": "1.2.3", "retries": 2, "nested": { "on": true } }
// @filename: example.ts
// A `.json` module's exports come from the value it holds: the whole value as
// the default export and one named export per top-level property, both widened
// the way tsc widens them.
import info, { version, retries } from "./data.json";
export const v: string = version;
export const r: number = retries;
export const n: boolean = info.nested.on;
