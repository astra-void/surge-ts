import "pkg";

declare module "pkg" {
  export function makeClient(id: string): { id: string };
}
