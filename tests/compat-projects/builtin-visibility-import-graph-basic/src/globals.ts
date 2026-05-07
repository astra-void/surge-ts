type Bucket = Record<"a", string>;

export const promiseValue: Promise<string> = null as any;
export const bucketValue: Bucket = { a: "ok" };
export const arrayValue: Array<number> = Array.from([1, 2]);
export const mathValue = Math.floor(1.9);
export const dateValue = Date.now();
export const numberValue = Number("42");
export const jsonValue = JSON.stringify({ ok: true });
export const globalThisValue = globalThis;
export const mapValue: Map<string, number> = null as any;
export const mapCtor = Map;
export const uint8ArrayValue: Uint8Array = null as any;
export const uint8ArrayCtor = Uint8Array;
export const isNaNValue = isNaN(42);
export const objectValue = Object.keys({ ok: true });
