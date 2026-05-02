function getState(): string {
  return "ok";
}

function read(): void {
  let store: { getState: () => string };
  store = { getState };
  store.getState();
}
