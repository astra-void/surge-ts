// @surge-compare: messages
// The namespace value object models only the member *set*, so the call must
// resolve through the qualified `ns.member` binding to infer `U`.
export namespace util {
  export const arrayToEnum = <T extends string, U extends [T, ...T[]]>(
    items: U
  ): { [k in U[number]]: k } => ({}) as any;
}
const codes = util.arrayToEnum(["a", "b"]);
export const bad: 1 = codes.a;
