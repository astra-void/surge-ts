type Constructor<T extends object = object> = new (...args: any[]) => T;

export function constructorName<E extends Error>(ctor: Constructor<E>): string {
  return ctor.name;
}

declare class Widget {
  value: number;
}

export const staticName: string = Widget.name;
export const staticLength: number = Widget.length;

interface Callable {
  (input: string): number;
}
declare const callable: Callable;
export const callableName: string = callable.name;
export const bound = callable.bind(null);

export const wrongName: number = Widget.name;
