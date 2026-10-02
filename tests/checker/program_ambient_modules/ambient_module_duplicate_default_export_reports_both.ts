// @filename: src/index.ts
import value from "dup-default-pkg"; let ok: string = value;
// @filename: types/ambient.d.ts

            declare module "dup-default-pkg" {
                export default "first";
            }

            declare module "dup-default-pkg" {
                export default 123;
            }
            
