import { imported } from "./other";

const text = 2..toFixed(0);
const joined = "a" + "b";
const { picked } = { picked: "s" as string };
let counter = 1;
declare const count: number;

export enum Computed {
  A = `${text}`,
  B = "2" + text,
  C = text.length,
  D = joined,
  E = D + "c",
  F = picked,
  G = counter,
  H = () => 4,
  I = "q" as string,
  J = count * 2,
  K = Computed.C + text,
  L = imported,
  M = undefined,
  N = Symbol(),
}

{
  let Infinity = {};
  enum Shadowed { X = Infinity }
}

export function inner(flag: boolean) {
  enum Local { A = flag ? 1 : 2, B = String(flag) }
  return Local;
}

export namespace Space {
  const local = "l";
  export enum InSpace { A = local, B = local.length, C = [local][0] }
}

export enum Checked {
  A = "a" - "b",
  B = missing,
  C = Checked.Nope,
  D = Computed,
}

declare enum AmbientComputed { A = "foo".length }
export const enum Overflow { A = NaN, B = Infinity, C = -Infinity }
export enum AfterLength { A = "ab".length, B }
export enum Flags { A = 1 << 0, B = 1 << 1, AB = A | B, Text = `${AB}-x` }
