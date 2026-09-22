declare function load(): Promise<number>;
export async function tryCatch() {
  let value: number;
  try {
    value = await load();
  } catch {
    return;
  }
  return value;
}

interface Box {
  v: number;
}
export function truthy(box: Box | false | 0 | '') {
  if (box) {
    return box.v;
  }
  return 0;
}

type Result =
  | { type: 'started' }
  | { type: 'data'; id?: string; data: unknown };
export function reduced(data: unknown) {
  let result: null | Result;
  result = { type: 'data', data };
  result.id = 'x';
  return result;
}
export function orNarrowing(env: { result: Result | { type?: undefined; id?: string } }) {
  if ((!env.result.type || env.result.type === 'data') && env.result.id) {
    return env.result.id;
  }
  return '';
}

const FULFILLED = 0;
const REJECTED = 1;
export function exhaustive(status: 0 | 1) {
  switch (status) {
    case FULFILLED:
      return 'a';
    case REJECTED:
      return 'b';
  }
}
export function partial(status: 0 | 1 | 2) {
  switch (status) {
    case FULFILLED:
      return 'a';
    case REJECTED:
      return 'b';
  }
}

type ChunkIndex = number & { __chunkIndex: true };
export function brand() {
  let counter = 0 as ChunkIndex;
  const next = counter++;
  const text: string = counter;
  return [next, text, counter.toFixed(1)];
}

type Encoded = [[unknown] | [], ...[string, number][]];
export function openTuple(value: Encoded) {
  const [[data], ...rest] = value;
  return [data, rest, ['b', 'a'].toSorted()];
}

declare const skipToken: unique symbol;
type Fn = (input: number) => string;
export function uniqueSymbol(fn: Fn | typeof skipToken | undefined) {
  return fn && fn !== skipToken ? (input: number) => fn.call({}, input) : undefined;
}

function isAsyncIterable<T>(value: unknown): value is AsyncIterable<T> {
  return typeof value === 'object' && value !== null && Symbol.asyncIterator in value;
}
declare function consume(iterable: AsyncIterable<unknown>): void;
export function predicateOnUnknown(value: unknown) {
  if (isAsyncIterable(value)) {
    consume(value);
  }
}
