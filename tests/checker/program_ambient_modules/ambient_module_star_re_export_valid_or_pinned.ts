// @filename: src/index.ts
import { User, value } from "barrel-star-pkg"; let user: User = { name: value };
// @filename: types/ambient.d.ts

            declare module "source-pkg" {
                export interface User { name: string; }
                export const value: string;
            }

            declare module "barrel-star-pkg" {
                export * from "source-pkg";
            }
            
