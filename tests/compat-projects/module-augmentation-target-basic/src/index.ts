declare module "does-not-exist" { export const x: number; }
declare module "./missing" { export const y: number; }
declare module "./present" { export const z: number; }
import { p } from "./present";
export const q = p;
