export {};

declare const source: { x: number; m(): void };
declare const either: { x: number } | { x: string; y: 1 };
declare const optional: { x?: number };
const x = 1;
const o1 = { x: 1, ...source };
const o2 = { x, ...source };
const o3 = { m() {}, ...source };
const o4 = { x: 1, ...either };
const o5 = { y: 1, ...either };
const o6 = { x: 1, ...optional };
const o7 = { x: 1, y: 2, ...source, ...source };
const o8: { x: number } = { x: 2, ...source };
const o9 = { ...source, x: 1 };

interface Options {
  url?: string;
  port?: number;
}
declare const connection: { url?: string } & Options;
if (connection.url !== undefined) {
  const { url, ...config } = connection;
  const merged = { url, ...config };
  const check: number = config;
}
declare const pair: { url: string; port: number };
const { url, ...rest } = pair;
const back = { url, ...rest };
const restCheck: number = rest;
function parameter({ url, ...others }: { url: string; port: number }) {
  const again = { url, ...others };
  const typed: number = others;
}
declare const tagged: { kind: 1; left: string } | { kind: 2; right: number } | undefined;
if (tagged) {
  const { kind, ...fields } = tagged;
  const fieldCheck: number = fields;
}

function generic<T extends { x: number }>(t: T) {
  return { x: 1, ...t };
}
function optionalGeneric<T extends { x?: number }>(t: T) {
  return { x: 1, ...t };
}
const arrowGeneric = <U extends { z: 1 }>(u: U) => ({ z: 1, ...u });
