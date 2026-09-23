declare module "legacy-callable" {
  function legacy(): number;
  namespace legacy {
    const version: string;
  }
  export = legacy;
}

declare module "named-only" {
  export const count: number;
}

declare module "assert-like" {
  export const strict: { ok(value: unknown): void; label: string };
}

declare module "assert-like/strict" {
  import { strict } from "assert-like";
  export = strict;
}
