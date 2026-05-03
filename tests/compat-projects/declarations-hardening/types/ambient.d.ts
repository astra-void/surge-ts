declare type ID = string;

declare interface User {
  name: string;
}

declare module "pkg-default" {
  export const value: string;
  export default value;
}

declare module "pkg-default-function" {
  export default function getName(): string;
}

declare module "pkg-ns" {
  export const value: string;
  export function getName(): string;
  export interface User {
    name: string;
  }
}

declare module "source-pkg" {
  export interface User {
    name: string;
  }

  export const value: string;
}

declare module "barrel-pkg" {
  export { User, value } from "source-pkg";
}

declare module "barrel-type-pkg" {
  export type { User } from "source-pkg";
}

declare module "barrel-star-pkg" {
  export * from "source-pkg";
}

declare module "merge-pkg" {
  export const a: string;
}

declare module "merge-pkg" {
  export const b: number;
}
