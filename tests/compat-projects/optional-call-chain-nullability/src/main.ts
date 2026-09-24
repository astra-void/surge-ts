declare const always: { run(): number; make(): () => number; items: string[] };
declare const maybe: { run(): number } | undefined;
declare const fn: () => string;
declare const maybeFn: (() => string) | undefined;

export const a: number = always?.run();
export const b: number = maybe?.run();
export const c: string = fn?.();
export const d: string = maybeFn?.();
export const e: number = (always?.make())();
export const f: string[] = always?.items.map((item) => item);
