interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

function read(): string {
  store.getState();
  let store: Store = { getState };
  return "ok";
}