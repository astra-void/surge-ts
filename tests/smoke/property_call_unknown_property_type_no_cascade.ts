interface Store {
  getState: unknown;
}

function getState(): string {
  return "ok";
}

let store: Store = { getState };
store.getState();