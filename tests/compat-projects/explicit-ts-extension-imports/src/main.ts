import { value } from "./value.ts";
import { declared } from "./declared.d.ts";
import type { declared as Declared } from "./declared.d.ts";
import { view } from "./view.tsx";
import { flavor } from "./flavor.mts";
import { absent } from "./absent.d.ts";

export const total: number = value + declared + view;
export const wrongFlavor: number = flavor;
export type Kept = typeof Declared;
export const copy = absent;
