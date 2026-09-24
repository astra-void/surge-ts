import * as counter from "./counter.cjs";
import * as make from "./make.cjs";
import * as esm from "./esm.mjs";

counter.default.count.toFixed();
counter.count.toFixed();
make.default().toFixed();
esm.default.toUpperCase();
const wrong: number = esm.default;
const missing = esm.flag.default;
