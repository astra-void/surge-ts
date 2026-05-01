interface Store {
  getState: () => string;
}

let store: Store = {};
