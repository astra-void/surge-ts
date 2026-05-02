function take(): string {
  return "ok";
}

interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

let store: Store = { getState };
let left: string = take();
let right: string = store.getState();
