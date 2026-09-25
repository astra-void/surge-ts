import type { Box } from "./box";

export function construct<A, E>(Ctor: typeof Box): [A, E] {
  return new Ctor<A, E>().get();
}

export function readUnknown({ x }: unknown) {
  return x;
}

export function iterateUnknown([first]: unknown) {
  return first;
}

export function readMissing({ id, name }: { id: number }) {
  return [id, name];
}

export function pastTheEnd([a, b, c]: [number, string]) {
  return [a, b, c];
}

export function caught() {
  try {
    return 1;
  } catch ({ message }) {
    return message;
  }
}

class Base {
  protected secret = "";
  private hidden = 0;
}

export function nested() {
  class Local {
    method(base: Base): number {
      const wrong: number = "text";
      return base.secret.length + base.hidden + wrong;
    }
  }
  return new Local();
}

var unassigned: string;
var count: number;
export const keyed = {
  [unassigned]: 0,
  [count]: count,
};

interface Caller {
  call(value: number, ...rest: string[]): Caller;
}
var caller: Caller;
var parts: string[];
(caller.call)(1, ...parts);
