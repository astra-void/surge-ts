export namespace M {
  export function one(): number {
    return 1;
  }
}
export namespace M {
  export function two(): number {
    return 2;
  }
}

export const total = M.one() + M.two();
