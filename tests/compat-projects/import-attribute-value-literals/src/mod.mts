import * as template from "./dep.mjs" with { field: `a` };
import * as computed from "./dep.mjs" with { type: "json", field: 0..toString() };
export const values = [template.value, computed.value];
