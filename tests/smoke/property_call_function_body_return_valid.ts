interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

function read(store: Store): string {
  return store.getState();
}