function getState(): string {
  return "ok";
}

let store = { getState };
let values = [store.getState()];
