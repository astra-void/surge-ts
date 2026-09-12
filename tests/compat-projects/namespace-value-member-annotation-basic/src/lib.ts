import type { Checker } from "./checker";

export declare namespace shapes {
  interface Box {
    width: number;
  }
  let Box: Checker<Box>;
  let count: number;

  namespace nested {
    let label: string;
  }
}
