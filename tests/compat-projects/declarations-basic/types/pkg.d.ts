declare module "pkg" {
  export interface PackageUser {
    name: string;
  }

  export type PackageID = string;
  export const value: string;
  export function getName(user: PackageUser): string;
}

declare module "pkg/subpath" {
  export const subValue: number;
}
