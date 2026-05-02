interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

function read(store: Store): number {
  return store.getState();
}