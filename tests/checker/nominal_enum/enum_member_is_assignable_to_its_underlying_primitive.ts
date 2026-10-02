// An enum type is displayed by the enum's name. The nominal wrapper carries that
// without disturbing the literal-union payload underneath, so everything an enum
// member could do before it still does.
enum Color { Red = 1 }
enum Names { Wide = "wide" }
declare const c: Color.Red;
declare const n: Names.Wide;
export const a: number = c;
export const b: string = n;
