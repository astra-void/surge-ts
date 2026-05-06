import dep from "dep-incomplete";
import { missingLocal } from "./local";
import { missingSource } from "./source";

export const currentDep = dep;
export const currentLocal = missingLocal;
export const currentSource = missingSource;
