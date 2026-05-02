function getState(): string {
  return "ok";
}

function read(): void {
  store.getState();
  let store: { getState: () => string } = { getState };
}
