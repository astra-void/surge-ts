import { Missing } from "missing-package";
import type { AlsoMissing } from "missing-package";
declare const value: Missing<number>;
export const other: AlsoMissing<string, boolean> = value;
