interface Store {
  getState: () => number;
}

function getState(): number {
  return 1;
}

let store: Store = {
  getState,
};

let fn: () => string = store.getState;
