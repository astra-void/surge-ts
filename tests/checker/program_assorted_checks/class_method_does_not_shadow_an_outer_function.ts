// A method name is a member, not a lexical binding: the bare call inside the
// body must reach the module-level `helper`.
export function helper(a: string, b: number): string {
  return a;
}
export class C {
  helper(a: string): string {
    return helper(a, 1);
  }
}
