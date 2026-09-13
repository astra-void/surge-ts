declare function read(path: string, options?: { encoding?: null; flag?: string } | null): Uint8Array;
declare function read(path: string, options: { encoding: 'utf8' | 'ascii'; flag?: string } | 'utf8' | 'ascii'): string;

export const bytes: Uint8Array = read('a');
export const text: string = read('a', 'utf8');
export const alsoText: string = read('a', { encoding: 'ascii' });

type Defined<TData> = { key: string; fn: () => TData; initialData: TData };
type Undefined<TData> = { key: string; fn: () => TData };

declare function query<TData = unknown>(options: Defined<TData>): { data: TData };
declare function query<TData = unknown>(options: Undefined<TData>): { data: TData | undefined };

export const defined: { data: string } = query({ key: 'k', fn: () => 'v', initialData: 'v' });
export const maybe: { data: string | undefined } = query<string>({ key: 'k', fn: () => 'v' });

declare function run(kind: 'a', callback: (value: number) => void): 'first';
declare function run(kind: 'b', callback: (value: string) => void): 'second';

export const second: 'second' = run('b', (value) => value.length);
