interface Store {
  log: () => void;
}

function log(): void {}

let store: Store = { log };
store.log();
