// A callback is typed by whichever overload is picked, so it cannot pick: it
// is a wildcard, and the other arguments decide.
declare function run(kind: 'a', cb: (x: number) => void): 'first';
declare function run(kind: 'b', cb: (x: string) => void): 'second';
export const second: 'second' = run('b', (x) => x.length);
