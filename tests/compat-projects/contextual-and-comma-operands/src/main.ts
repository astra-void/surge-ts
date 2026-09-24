declare const enabled: true;
declare let callback: ((a: string) => number) | undefined;

export const viaComma: (a: string) => number = (0, a => a.length);
export const viaAnd: (a: string) => number = enabled && (a => a.length);
callback &&= (a => a.length);
callback ??= (0, a => a.length);
export const leftOfAnd: (a: string) => string = (a => a) && (b => b);
export const wrongComma: (a: string) => number = (0, (a: number) => a);
