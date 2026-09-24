import legacy from "legacy-callable";
import namedOnly from "named-only";
import strict from "assert-like/strict";

const called: string = legacy();
const count: string = namedOnly.count;
strict.ok(true);
const label: number = strict.label;
export {};
