declare function pick(value: string): string;
declare function pick(value: number): number;
pick(true);

declare function pad(text: string): string;
declare function pad(text: string, width: number): string;
pad(1, 2);
pad(1);

declare function route(path: string): void;
declare function route(path: string, handler: () => void, options: object): void;
route(42);

const picked: number = pick(1);
