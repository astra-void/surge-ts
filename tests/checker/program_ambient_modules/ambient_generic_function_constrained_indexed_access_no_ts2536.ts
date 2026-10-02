// @filename: src/index.ts
// An ambient `declare function` whose signature indexes a concrete type by a
// constrained type parameter (`K extends keyof EventMap` → `EventMap[K]`, as
// the lib `addEventListener` does) must not emit a false TS2536. The ambient
// collection path resolves this signature authoritatively (no body follows),
// so it must do so under the function's own type-parameter scope. Single-file
// checking always had that scope; this pins the project/ambient path.
export const x = 1;
// @filename: types/dom.d.ts
interface BaseMap { click: number; }
interface EventMap extends BaseMap { focus: string; }
declare function on<K extends keyof EventMap>(type: K, listener: (this: object, ev: EventMap[K]) => any): void;
declare function on(type: string, listener: () => void): void;
