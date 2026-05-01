interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

let store: Store = {
  getState,
};

let fn: () => string = store.getState;
