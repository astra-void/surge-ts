// @filename: box.ts
interface StoreApi<TState> { getState: () => TState; }
// @filename: index.ts
function getState(): number { return 123; } let store: StoreApi<string> = { getState: getState };
