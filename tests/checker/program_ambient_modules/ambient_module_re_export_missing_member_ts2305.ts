// @filename: src/index.ts
import { missing } from "barrel-pkg";
// @filename: types/ambient.d.ts

            declare module "source-pkg" {
                export const value: string;
            }

            declare module "barrel-pkg" {
                export { missing } from "source-pkg";
            }
            
