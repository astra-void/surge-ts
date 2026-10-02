// @filename: src/index.ts
// Reopened ambient module blocks merge their exported interfaces; the
// conflicting redeclaration of `name` is TS2717.
import type { User } from "dup-type-pkg"; let ok: User = { name: "Ada" };
// @filename: types/ambient.d.ts

            declare module "dup-type-pkg" {
                export interface User { name: string; }
            }

            declare module "dup-type-pkg" {
                export interface User { name: number; }
            }
            
