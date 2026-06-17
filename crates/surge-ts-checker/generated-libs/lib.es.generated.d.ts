// Generated from the local TypeScript lib sources. Do not edit by hand.

interface Array<T> {
  length: number;
  map<U>(callback: (value: T, index: number, array: T[]) => U): U[];
  find(callback: (value: T, index: number, array: T[]) => unknown): T | undefined;
  join(separator?: string): string;
  includes(value: T): boolean;
  push(...items: T[]): number;
}

interface ReadonlyArray<T> {
  length: number;
}

interface ArrayConstructor {
  from(value: unknown): any[];
}

declare const Array: ArrayConstructor;

interface Promise<T> {}

interface PromiseLike<T> {}

interface PromiseConstructor {
  resolve<T>(value: T): Promise<T>;
  all<T>(values: Promise<T>[]): Promise<T[]>;
}

declare const Promise: PromiseConstructor;

interface Map<K, V> {
  get(key: K): any;
  set(key: K, value: V): any;
  has(key: K): boolean;
  delete(key: K): boolean;
  clear(): void;
  size: number;
}

interface Uint8Array extends Array<number> {}

type Date = any;

interface String {
  replace(searchValue: string | RegExp, replaceValue: string): string;
  split(separator: string | RegExp): string[];
  slice(start?: number, end?: number): string;
  toLowerCase(): string;
  toUpperCase(): string;
  padStart(maxLength: number, fillString?: string): string;
  charCodeAt(index: number): number;
}

interface Number {
  toString(radix?: number): string;
}

interface Boolean {}

interface ObjectConstructor {
  keys(value: unknown): string[];
}

declare const Object: ObjectConstructor;

declare const Date: {
  now: () => number;
};

declare const Math: {
  floor: (value: number) => number;
  max: (a: number, b?: number, c?: number, d?: number) => number;
  min: (a: number, b?: number, c?: number, d?: number) => number;
  round: (value: number) => number;
};

declare const JSON: {
  stringify: (value: unknown) => string;
  parse: (value: string) => unknown;
};

declare function decodeURIComponent(encodedURIComponent: string): string;

declare function isNaN(value: unknown): boolean;

declare function Number(value?: unknown): number;
declare function String(value?: unknown): string;
declare function Boolean(value?: unknown): boolean;
declare function Map<K, V>(): Map<K, V>;
declare function Uint8Array(value?: unknown): Uint8Array;

/**
 * Make all properties in T optional
 */
type Partial<T> = {
  [P in keyof T]?: T[P];
};

/**
 * From T, pick a set of properties whose keys are in the union K
 */
type Pick<T, K extends keyof T> = {
  [P in K]: T[P];
};

type Record<K extends keyof any, T> = { [P in K]: T };

/**
 * Construct a type with the properties of T except for those in type K.
 */
type Omit<T, K extends keyof any> = Pick<T, Exclude<keyof T, K>>;

type Parameters<T> = unknown[];

type ReturnType<T> = unknown;
