interface Store {
  log: () => void;
}

function log(): void {}

let store: Store = { log };
let value: void = store.log();
