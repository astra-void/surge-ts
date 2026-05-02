interface Store {
  getState: (value: string) => string;
}

function getState(value: string): string {
  return "ok";
}

function read(store: Store): string {
  return store.getState(missing);
}
