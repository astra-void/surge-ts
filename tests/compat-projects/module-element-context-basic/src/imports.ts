function f() {
  import a from "./dep";
  import b = require("./dep");
  import c = N.x;
}
namespace N { export const x = 1; }
{
  import d from "./dep";
}
import e from "./dep";
namespace P {
  import g = N.x;
}
export {};
