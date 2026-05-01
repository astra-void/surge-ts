interface Store {
  getState: () => string;
}

function getState(): number {
  return 1;
}

let store: Store = {
  getState,
};
