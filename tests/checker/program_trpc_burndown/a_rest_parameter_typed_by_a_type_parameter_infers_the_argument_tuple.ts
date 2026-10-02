declare function f<E extends any[]>(n: number, ...args: E): void; f(1, [1, 2]); f(2, 'a', [3]); f(3); declare function g<T>(...args: T[]): T; const s: string = g(1, 2); const t: number = g('a', 'b');
