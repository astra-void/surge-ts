import { Other, imported } from "./other";
const lit = 2;
const strLit = "s";
let mutable = 1;
declare const dyn: number;
function fn() { return 1; }
export enum E1 { A = lit, B }
export enum E2 { A = strLit, B }
export enum E3 { A = mutable, B }
export enum E4 { A = Other.A, B }
export enum E5 { A = imported, B }
export enum E6 { A = fn(), B }
export enum E7 { A = `x`, B }
export enum E8 { A = 1 + 2, B, C = A | 4, D, E = "a" + "b", F }
export enum E9 { A = -1, B = ~A, C }
export const enum C1 { A = lit, B = mutable, C = fn(), D = Other.A, E = imported, F = dyn }
export enum E10 { A = E9.C, B }
export enum E11 { A = (2), B }
export enum E12 { A = lit as number, B }
export enum E13 { A = "a", B, C }
declare enum Amb { A = dyn, B }
export namespace NS { const inner = 1; export enum E { A = inner, B } }
