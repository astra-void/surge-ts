interface Store {
  getState: (value: string) => string;
}

function getState(value: string): string {
  return "ok";
}

let store: Store = { getState };
let value: string = store.getState(missing);
