// The nominal wrapper must not cost discriminant narrowing.
enum Names { Wide = "wide", Tall = "tall" }
interface W { kind: Names.Wide; w: number }
interface T { kind: Names.Tall; t: number }
export function area(v: W | T): number {
return v.kind === Names.Wide ? v.w : v.t;
}
