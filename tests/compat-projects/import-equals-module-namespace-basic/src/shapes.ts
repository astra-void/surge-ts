export declare namespace shapes {
  interface Box {
    width: number;
  }
  let Box: { check(value: unknown): boolean };
}

export interface Thing {
  kind: string;
}
export declare const thing: Thing;
