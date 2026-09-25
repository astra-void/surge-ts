import "./values" with { type: "json" };
import { a } from "./values" with { type: "json" };
import * as all from "./values" with {};
export { b } from "./values" with { type: "json" };
export * from "./values" with { type: "json" };
export * as again from "./values" with { type: "json" };

const loaded = import("./values");
const withOptions = import("./values", { with: { type: "json" } });
const empty = import();
const extra = import("./values", {}, {});

export { a, all, loaded, withOptions, empty, extra };
