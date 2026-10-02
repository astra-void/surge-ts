// @filename: src/index.ts
import { a, b } from "merge-pkg"; let okA: string = a; let okB: number = b;
// @filename: types/ambient.d.ts

            declare module "merge-pkg" {
                export const a: string;
            }

            declare module "merge-pkg" {
                export const b: number;
            }
            
