function getState(): string {
  return "ok";
}

let store: { getState: () => string } = { getState };
store.missing();