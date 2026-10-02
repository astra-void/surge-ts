// @filename: a.ts
interface Store { getState: () => string; }
// @filename: b.ts
let store: Store = { getState: () => "ok" }; let value: string = store.getState();
