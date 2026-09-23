namespace N {
  declare module "foo" {}
}
declare namespace M {
  module "c" {}
}
declare module "a" {
  module "b" {}
}
declare module "ok" {}
export {};
