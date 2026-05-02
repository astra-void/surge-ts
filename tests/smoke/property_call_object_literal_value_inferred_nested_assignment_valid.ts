interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

let store: Store = { getState };
let box = { nested: { value: store.getState() } };
let value: { value: string } = box.nested;
