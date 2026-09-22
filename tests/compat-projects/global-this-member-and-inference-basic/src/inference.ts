declare function pair<T>(a: T, b: T): T;

export const objects = pair({ foo: "a" }, { foo: "a", asd: 1 });
export const arrays = pair([{ foo: "a" }], [{ foo: "a", asd: 1 }]);

declare function annotated(value: { foo: string }): void;
annotated({ foo: "a", asd: 1 });

declare global {
  interface Window {
    __EXT__?: { connect: (param: number) => string };
  }
}

export type Connect = Window extends { __EXT__?: infer T }
  ? T
  : { connect: (param: unknown) => unknown };
export type ConnectParam = Parameters<Connect["connect"]>[0];
export const param: string = null as unknown as ConnectParam;
