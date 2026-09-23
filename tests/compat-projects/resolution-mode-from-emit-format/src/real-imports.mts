import { fromCjs, fromEsm } from "dual";
import { esmOnly } from "esm-only";

export const values = [fromCjs, fromEsm, esmOnly];
