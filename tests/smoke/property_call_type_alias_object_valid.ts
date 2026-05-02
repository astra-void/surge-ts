type Store = {
  getState: () => string;
};

function getState(): string {
  return "ok";
}

let store: Store = { getState };
let value: string = store.getState();