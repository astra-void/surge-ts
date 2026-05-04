import { FromTypings } from "typings-pkg";
const x: number = FromTypings; // Should be TS2322 (string not assignable to number)