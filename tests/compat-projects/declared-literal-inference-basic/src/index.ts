export {}
type Chunk = [number, 0, string] | [number, 1, boolean];

declare const chunks: Chunk[];
declare const chunk: Chunk;
declare const boxed: { v: Chunk };
declare const stream: AsyncGenerator<Chunk>;

declare function idArr<T>(x: T[]): T[];
declare function idBox<T>(x: { v: T }): T;
declare function idT<T>(x: T): T;
declare function passThrough<T>(src: AsyncIterable<T>): AsyncGenerator<T | symbol>;

const fromArray: Chunk[] = idArr(chunks);
const fromMember: Chunk = idBox(boxed);
const fromSelf: Chunk = idT(chunk);
const fromIterable: AsyncIterable<Chunk | symbol, void> = passThrough(stream);

declare function take<T>(x: T[]): T;
const widened: number = take([1, 2, 3]);

declare function box<T>(x: { v: T }): T;
const widenedMember: string = box({ v: 'a' });
