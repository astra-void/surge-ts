// @filename: src/index.ts
on("click", (event) => { const value: number = event; });
// @filename: types/dom.d.ts
interface EventMap { click: number; }
declare function on<K extends keyof EventMap>(type: K, listener: (event: EventMap[K]) => void): void;
declare function on(type: string, listener: (() => void) | object): void;
