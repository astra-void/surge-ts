// @surge-args: --stubExternalModules
// @surge-expect: none
// @filename: src/index.ts
import { User } from "barrel-pkg";
// @filename: types/ambient.d.ts

                declare module "barrel-pkg" {
                    export { User } from "missing-pkg";
                }
                
