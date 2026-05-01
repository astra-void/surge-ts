interface Store {
  getState: () => Missing;
}

function getState(): string {
  return "ok";
}

let store: Store = {
  getState,
};
