function getState(): string {
  return "ok";
}

let store: { getState: () => string } = { getState };
let value: number = store.getState();