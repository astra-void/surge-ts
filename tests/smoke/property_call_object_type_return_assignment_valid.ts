function getState(): string {
  return "ok";
}

let store: { getState: () => string } = { getState };
let value: string = store.getState();