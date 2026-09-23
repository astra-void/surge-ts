function f() {
  declare module "foo" {}
  declare global {}
  namespace N {}
  declare namespace M {}
}
{
  declare module "bar" {}
  namespace Q {}
}
declare module "ok" {}
declare global {}
namespace Ok {
  namespace Nested {}
  export namespace Exported {}
}
namespace A.B.C {}
export {};
