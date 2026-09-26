export * from "assigned";
export * from "named";
import type * as values from "./values";
import type * as absent from "./absent";
values;
values.origin;
absent;
export const holder = { values };
export type Size = values.Shape["size"];
