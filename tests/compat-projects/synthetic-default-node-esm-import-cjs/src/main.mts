import counter from "./counter.cjs";
import esm from "./esm.mjs";
import esmDefault from "./esmDefault.mjs";

counter.count.toFixed();
counter.default();
counter();
esm;
esmDefault().toUpperCase();
