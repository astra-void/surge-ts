declare global {
  export {};
  export * from "dep";
  export default N.x;
  export const ok: number;
  export interface Ok2 {}
  import a from "dep";
  import b = require("dep");
  import c = N.x;
  const y: number, z: number;
}
namespace N { export const x = 1; }
export {};
