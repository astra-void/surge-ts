// @filename: box.ts
interface StoreApi<TState> { getState: () => TState; }
// @filename: index.ts
function getState(): string { return "ok"; } let store: StoreApi<string> = { getState: getState };
