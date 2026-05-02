interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

let store: Store = { getState };
let value: string = true ? store.getState() : "fallback";
