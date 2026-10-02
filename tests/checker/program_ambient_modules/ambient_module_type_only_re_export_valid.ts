// @filename: src/index.ts
import type { User } from "barrel-type-pkg"; let user: User = { name: "Ada" };
// @filename: types/ambient.d.ts

            declare module "source-pkg" {
                export interface User { name: string; }
            }

            declare module "barrel-type-pkg" {
                export type { User } from "source-pkg";
            }
            
