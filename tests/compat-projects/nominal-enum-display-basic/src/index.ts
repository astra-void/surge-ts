export enum Exported { Red = 1, Green = 2 }
enum Local { A = 1, B = 2 }

declare const exportedMember: Exported.Red;
declare const localMember: Local.A;

export const fromExported: string = exportedMember;
export const fromLocal: string = localMember;

export const toNumber: number = exportedMember;

enum Names { Wide = "wide", Tall = "tall" }
interface W { kind: Names.Wide; w: number }
interface T { kind: Names.Tall; t: number }
export function area(v: W | T): number {
  return v.kind === Names.Wide ? v.w : v.t;
}
