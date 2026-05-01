interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

let store: Store = {
  getState,
  extra: 1,
};
