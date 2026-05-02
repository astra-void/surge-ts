interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

let store: Store = { getState };
let box = { value: store.getState() };
let value: number = box.value;
