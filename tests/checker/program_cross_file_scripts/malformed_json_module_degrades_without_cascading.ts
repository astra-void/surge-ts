// @filename: data.json
{ 'version': 1 }
// @filename: example.ts
// A `.json` file that does not parse is still a module. Reporting its importer
// as unresolved would be a worse answer than an unmodelled value, and surge
// does not report JSON syntax errors, so the value degrades and nothing
// cascades. tsc reports the syntax error on the JSON file itself.
import info, { version } from "./data.json";
export const v: number = version;
export const anything = info.whatever;
